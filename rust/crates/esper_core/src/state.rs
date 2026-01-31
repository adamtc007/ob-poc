//! Navigation state machine.
//!
//! DroneState is the central state machine that tracks:
//! - Current chamber and context stack
//! - Camera position and zoom
//! - Taxonomy selection and expansion
//! - Navigation phase (for render optimization)

use crate::effect::EffectSet;
use crate::fault::Fault;
use crate::phase::NavigationPhase;
use crate::stack::{ContextFrame, ContextStack};
use crate::verb::{ChamberId, EntityId, NodeIdx, Verb};
use esper_snapshot::{ChamberSnapshot, Vec2, WorldSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Navigation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NavigationMode {
    /// Spatial navigation: pan, zoom, focus.
    #[default]
    Spatial,

    /// Structural navigation: tree traversal.
    Structural,
}

/// Camera state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraState {
    /// Target position (where camera should be).
    pub target: Vec2,

    /// Current position (for animation lerp).
    pub current: Vec2,

    /// Zoom level (1.0 = 100%).
    pub zoom: f32,

    /// Target zoom (for animation).
    pub target_zoom: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            target: Vec2::ZERO,
            current: Vec2::ZERO,
            zoom: 1.0,
            target_zoom: 1.0,
        }
    }
}

impl CameraState {
    /// Check if camera is animating.
    pub fn is_animating(&self) -> bool {
        let pos_diff = self.target.distance(self.current);
        let zoom_diff = (self.target_zoom - self.zoom).abs();
        pos_diff > 0.1 || zoom_diff > 0.01
    }

    /// Snap to target (no animation).
    pub fn snap(&mut self) {
        self.current = self.target;
        self.zoom = self.target_zoom;
    }
}

/// Taxonomy (tree navigation) state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaxonomyState {
    /// Path from root to current focus.
    pub focus_path: Vec<NodeIdx>,

    /// Current depth in the tree.
    pub current_depth: usize,

    /// Currently selected node (if any).
    pub selection: Option<NodeIdx>,

    /// Set of expanded nodes.
    pub expanded: HashSet<NodeIdx>,

    /// Scroll offset for long lists.
    pub scroll_offset: f32,

    /// Preview target (hover state).
    pub preview_target: Option<NodeIdx>,

    /// Tick of last navigation action.
    pub last_nav_tick: u64,

    /// Animation progress [0, 1].
    pub focus_t: f32,
}

impl TaxonomyState {
    /// Reset to initial state.
    pub fn reset(&mut self) {
        self.focus_path.clear();
        self.current_depth = 0;
        self.selection = None;
        self.expanded.clear();
        self.scroll_offset = 0.0;
        self.preview_target = None;
        self.focus_t = 0.0;
    }

    /// Check if a node is expanded.
    pub fn is_expanded(&self, node: NodeIdx) -> bool {
        self.expanded.contains(&node)
    }

    /// Expand a node.
    pub fn expand(&mut self, node: NodeIdx) {
        self.expanded.insert(node);
    }

    /// Collapse a node.
    pub fn collapse(&mut self, node: NodeIdx) {
        self.expanded.remove(&node);
    }

    /// Toggle expansion.
    pub fn toggle_expand(&mut self, node: NodeIdx) {
        if self.is_expanded(node) {
            self.collapse(node);
        } else {
            self.expand(node);
        }
    }
}

/// LOD (Level of Detail) state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LodState {
    /// Current LOD tier index.
    pub current_tier: usize,

    /// Counts of entities at each LOD level.
    pub icon_count: usize,
    pub label_count: usize,
    pub full_count: usize,
}

/// Main navigation state machine.
///
/// # Architecture
///
/// DroneState is updated in `execute()` and read by the renderer.
/// It contains no rendering logic - pure state transitions.
///
/// # egui Compliance
///
/// Per egui rules:
/// - Update DroneState in `update()` (before render)
/// - Read DroneState in `ui()` (during render)
/// - Never mutate during render
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneState {
    /// Current navigation mode.
    pub mode: NavigationMode,

    /// Current chamber ID.
    pub current_chamber: ChamberId,

    /// Context stack for nested navigation.
    pub context_stack: ContextStack,

    /// Camera state.
    pub camera: CameraState,

    /// Taxonomy (tree) state.
    pub taxonomy: TaxonomyState,

    /// LOD state.
    pub lod: LodState,

    /// Current frame tick (updated externally).
    pub tick: u64,
}

impl Default for DroneState {
    fn default() -> Self {
        Self {
            mode: NavigationMode::default(),
            current_chamber: 0,
            context_stack: ContextStack::new(),
            camera: CameraState::default(),
            taxonomy: TaxonomyState::default(),
            lod: LodState::default(),
            tick: 0,
        }
    }
}

impl DroneState {
    /// Create a new DroneState.
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute a verb, returning effects.
    ///
    /// This is the main entry point for all navigation commands.
    pub fn execute(&mut self, verb: Verb, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        if world.chambers.is_empty() {
            return Err(Fault::EmptyWorld);
        }

        match verb {
            // Spatial navigation
            Verb::PanBy { dx, dy } => self.pan_by(dx, dy),
            Verb::PanTo { x, y } => self.pan_to(x, y),
            Verb::Zoom(factor) => self.zoom(factor),
            Verb::ZoomFit => self.zoom_fit(world),
            Verb::ZoomTo(level) => self.zoom_to(level),
            Verb::Center => self.center(world),
            Verb::Stop => self.stop(),
            Verb::Enhance => self.enhance(),
            Verb::Reduce => self.reduce(),

            // Cross-chamber
            Verb::DiveInto(door_id) => self.dive_into(door_id, world),
            Verb::PullBack => self.pull_back(),
            Verb::Surface => self.surface(),

            // Structural navigation
            Verb::Ascend => self.ascend(world),
            Verb::Descend => self.descend(world),
            Verb::DescendTo(node) => self.descend_to(node, world),
            Verb::Next => self.next(world),
            Verb::Prev => self.prev(world),
            Verb::First => self.first(world),
            Verb::Last => self.last(world),
            Verb::Expand => self.expand_node(),
            Verb::Collapse => self.collapse_node(),
            Verb::Root => self.go_root(),

            // Selection
            Verb::Select(node) => self.select(node, world),
            Verb::Focus(entity) => self.focus(entity, world),
            Verb::Track(entity) => self.track(entity, world),
            Verb::Preview(node) => self.preview(node),
            Verb::ClearPreview => self.clear_preview(),

            // Mode
            Verb::ModeSpatial => self.mode_spatial(),
            Verb::ModeStructural => self.mode_structural(),
            Verb::ModeToggle => self.mode_toggle(),

            // Special
            Verb::Noop => Ok(EffectSet::NONE),
        }
    }

    /// Get current navigation phase based on tick.
    pub fn phase(&self) -> NavigationPhase {
        NavigationPhase::update(self.tick, self.taxonomy.last_nav_tick)
    }

    /// Get current chamber from world.
    pub fn chamber<'a>(&self, world: &'a WorldSnapshot) -> Option<&'a ChamberSnapshot> {
        world.chambers.iter().find(|c| c.id == self.current_chamber)
    }

    // =========================================================================
    // SPATIAL NAVIGATION
    // =========================================================================

    fn pan_by(&mut self, dx: f32, dy: f32) -> Result<EffectSet, Fault> {
        self.camera.target.x += dx;
        self.camera.target.y += dy;
        self.mark_navigation();
        Ok(EffectSet::CAMERA_CHANGED | EffectSet::PHASE_RESET)
    }

    fn pan_to(&mut self, x: f32, y: f32) -> Result<EffectSet, Fault> {
        self.camera.target = Vec2::new(x, y);
        self.mark_navigation();
        Ok(EffectSet::CAMERA_CHANGED | EffectSet::PHASE_RESET)
    }

    fn zoom(&mut self, factor: f32) -> Result<EffectSet, Fault> {
        self.camera.target_zoom *= factor;
        self.camera.target_zoom = self.camera.target_zoom.clamp(0.1, 10.0);
        self.mark_navigation();
        Ok(EffectSet::CAMERA_CHANGED | EffectSet::PHASE_RESET)
    }

    fn zoom_fit(&mut self, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        if let Some(chamber) = self.chamber(world) {
            self.camera.target = chamber.bounds.center();
            // Calculate zoom to fit (simplified - renderer should provide viewport)
            self.camera.target_zoom = 1.0;
        }
        self.mark_navigation();
        Ok(EffectSet::CAMERA_CHANGED | EffectSet::PHASE_RESET)
    }

    fn zoom_to(&mut self, level: f32) -> Result<EffectSet, Fault> {
        self.camera.target_zoom = level.clamp(0.1, 10.0);
        self.mark_navigation();
        Ok(EffectSet::CAMERA_CHANGED | EffectSet::PHASE_RESET)
    }

    fn center(&mut self, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        if let Some(selection) = self.taxonomy.selection {
            if let Some(chamber) = self.chamber(world) {
                if let Some(pos) = chamber.position(selection as usize) {
                    self.camera.target = pos;
                }
            }
        } else if let Some(chamber) = self.chamber(world) {
            self.camera.target = chamber.bounds.center();
        }
        self.mark_navigation();
        Ok(EffectSet::CAMERA_CHANGED | EffectSet::PHASE_RESET)
    }

    fn stop(&mut self) -> Result<EffectSet, Fault> {
        self.camera.snap();
        Ok(EffectSet::CAMERA_CHANGED | EffectSet::SNAP_TRANSITION)
    }

    fn enhance(&mut self) -> Result<EffectSet, Fault> {
        self.zoom(1.5)
    }

    fn reduce(&mut self) -> Result<EffectSet, Fault> {
        self.zoom(0.67)
    }

    // =========================================================================
    // CROSS-CHAMBER NAVIGATION
    // =========================================================================

    fn dive_into(&mut self, door_id: u32, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        let chamber = self
            .chamber(world)
            .ok_or(Fault::ChamberNotFound(self.current_chamber))?;

        let door = chamber
            .doors
            .iter()
            .find(|d| d.id == door_id)
            .ok_or(Fault::DoorNotFound(door_id))?;

        // Check for cycle
        if door.target_chamber_id == self.current_chamber {
            return Err(Fault::CyclicReference(door.target_chamber_id));
        }

        // Push current context
        let frame = ContextFrame::new(self.current_chamber, self.camera.current, self.camera.zoom)
            .with_selection(self.taxonomy.selection);

        self.context_stack.push(frame)?;

        // Switch to target chamber
        self.current_chamber = door.target_chamber_id;

        // Reset camera to target chamber's default
        if let Some(target_chamber) = world
            .chambers
            .iter()
            .find(|c| c.id == door.target_chamber_id)
        {
            self.camera.target = target_chamber.default_camera.center;
            self.camera.target_zoom = target_chamber.default_camera.zoom;

            // Focus on target entity if specified
            if door.has_target_entity() {
                if let Some(idx) = target_chamber.find_entity(door.target_entity_id) {
                    self.taxonomy.selection = Some(idx as u32);
                    if let Some(pos) = target_chamber.position(idx) {
                        self.camera.target = pos;
                    }
                }
            }
        }

        // Reset taxonomy
        self.taxonomy.reset();
        self.mark_navigation();

        Ok(EffectSet::CHAMBER_CHANGED
            | EffectSet::CONTEXT_PUSHED
            | EffectSet::CAMERA_CHANGED
            | EffectSet::SNAP_TRANSITION
            | EffectSet::LOD_MODE_RESET
            | EffectSet::PHASE_RESET)
    }

    fn pull_back(&mut self) -> Result<EffectSet, Fault> {
        let frame = self.context_stack.pop()?;

        self.current_chamber = frame.chamber_id;
        self.camera.target = frame.camera_pos;
        self.camera.target_zoom = frame.camera_zoom;
        self.taxonomy.selection = frame.selection;

        self.mark_navigation();

        Ok(EffectSet::CHAMBER_CHANGED
            | EffectSet::CONTEXT_POPPED
            | EffectSet::CAMERA_CHANGED
            | EffectSet::SNAP_TRANSITION
            | EffectSet::LOD_MODE_RESET
            | EffectSet::PHASE_RESET)
    }

    fn surface(&mut self) -> Result<EffectSet, Fault> {
        if self.context_stack.is_empty() {
            return Ok(EffectSet::NONE);
        }

        // Pop all contexts
        let mut first_frame = None;
        while let Ok(frame) = self.context_stack.pop() {
            first_frame = Some(frame);
        }

        if let Some(frame) = first_frame {
            self.current_chamber = frame.chamber_id;
            self.camera.target = frame.camera_pos;
            self.camera.target_zoom = frame.camera_zoom;
            self.taxonomy.selection = frame.selection;
        }

        self.mark_navigation();

        Ok(EffectSet::CHAMBER_CHANGED
            | EffectSet::CONTEXT_POPPED
            | EffectSet::CAMERA_CHANGED
            | EffectSet::SNAP_TRANSITION
            | EffectSet::LOD_MODE_RESET
            | EffectSet::PHASE_RESET)
    }

    // =========================================================================
    // STRUCTURAL NAVIGATION
    // =========================================================================

    fn ascend(&mut self, _world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        // Pop the parent node from the focus path
        let parent = self.taxonomy.focus_path.pop().ok_or(Fault::NoParent)?;

        self.taxonomy.current_depth = self.taxonomy.focus_path.len();
        self.taxonomy.selection = Some(parent);
        self.mark_navigation();

        Ok(EffectSet::TAXONOMY_CHANGED | EffectSet::SCROLL_ADJUST | EffectSet::PHASE_RESET)
    }

    fn descend(&mut self, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        let selection = self.taxonomy.selection.ok_or(Fault::NoSelection)?;
        let chamber = self
            .chamber(world)
            .ok_or(Fault::ChamberNotFound(self.current_chamber))?;

        let first_child = chamber.first_child_idx(selection as usize);
        let child_idx = first_child.ok_or(Fault::NoChildren(selection))?;

        self.taxonomy.focus_path.push(selection);
        self.taxonomy.selection = Some(child_idx as u32);
        self.taxonomy.current_depth = self.taxonomy.focus_path.len();
        self.taxonomy.expand(selection);
        self.mark_navigation();

        Ok(EffectSet::TAXONOMY_CHANGED | EffectSet::SCROLL_ADJUST | EffectSet::PHASE_RESET)
    }

    fn descend_to(&mut self, node: NodeIdx, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        let chamber = self
            .chamber(world)
            .ok_or(Fault::ChamberNotFound(self.current_chamber))?;

        if node as usize >= chamber.entity_count() {
            return Err(Fault::NodeNotFound(node));
        }

        // Add current selection to path if exists
        if let Some(current) = self.taxonomy.selection {
            self.taxonomy.focus_path.push(current);
            self.taxonomy.expand(current);
        }

        self.taxonomy.selection = Some(node);
        self.taxonomy.current_depth = self.taxonomy.focus_path.len();
        self.mark_navigation();

        Ok(EffectSet::TAXONOMY_CHANGED | EffectSet::SCROLL_ADJUST | EffectSet::PHASE_RESET)
    }

    fn next(&mut self, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        let selection = self.taxonomy.selection.ok_or(Fault::NoSelection)?;
        let chamber = self
            .chamber(world)
            .ok_or(Fault::ChamberNotFound(self.current_chamber))?;

        let next_idx = chamber
            .next_sibling_idx(selection as usize)
            .ok_or(Fault::NoNextSibling(selection))?;

        self.taxonomy.selection = Some(next_idx as u32);
        self.mark_navigation();

        Ok(EffectSet::TAXONOMY_CHANGED | EffectSet::SCROLL_ADJUST | EffectSet::PHASE_RESET)
    }

    fn prev(&mut self, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        let selection = self.taxonomy.selection.ok_or(Fault::NoSelection)?;
        let chamber = self
            .chamber(world)
            .ok_or(Fault::ChamberNotFound(self.current_chamber))?;

        let prev_idx = chamber
            .prev_sibling_idx(selection as usize)
            .ok_or(Fault::NoPrevSibling(selection))?;

        self.taxonomy.selection = Some(prev_idx as u32);
        self.mark_navigation();

        Ok(EffectSet::TAXONOMY_CHANGED | EffectSet::SCROLL_ADJUST | EffectSet::PHASE_RESET)
    }

    fn first(&mut self, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        let selection = self.taxonomy.selection.ok_or(Fault::NoSelection)?;
        let chamber = self
            .chamber(world)
            .ok_or(Fault::ChamberNotFound(self.current_chamber))?;

        // Walk backward to find first sibling
        let mut current = selection as usize;
        while let Some(prev) = chamber.prev_sibling_idx(current) {
            current = prev;
        }

        self.taxonomy.selection = Some(current as u32);
        self.mark_navigation();

        Ok(EffectSet::TAXONOMY_CHANGED | EffectSet::SCROLL_ADJUST | EffectSet::PHASE_RESET)
    }

    fn last(&mut self, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        let selection = self.taxonomy.selection.ok_or(Fault::NoSelection)?;
        let chamber = self
            .chamber(world)
            .ok_or(Fault::ChamberNotFound(self.current_chamber))?;

        // Walk forward to find last sibling
        let mut current = selection as usize;
        while let Some(next) = chamber.next_sibling_idx(current) {
            current = next;
        }

        self.taxonomy.selection = Some(current as u32);
        self.mark_navigation();

        Ok(EffectSet::TAXONOMY_CHANGED | EffectSet::SCROLL_ADJUST | EffectSet::PHASE_RESET)
    }

    fn expand_node(&mut self) -> Result<EffectSet, Fault> {
        let selection = self.taxonomy.selection.ok_or(Fault::NoSelection)?;
        self.taxonomy.expand(selection);
        Ok(EffectSet::TAXONOMY_CHANGED)
    }

    fn collapse_node(&mut self) -> Result<EffectSet, Fault> {
        let selection = self.taxonomy.selection.ok_or(Fault::NoSelection)?;
        self.taxonomy.collapse(selection);
        Ok(EffectSet::TAXONOMY_CHANGED)
    }

    fn go_root(&mut self) -> Result<EffectSet, Fault> {
        self.taxonomy.focus_path.clear();
        self.taxonomy.current_depth = 0;
        self.taxonomy.selection = Some(0); // Select first entity
        self.mark_navigation();

        Ok(EffectSet::TAXONOMY_CHANGED | EffectSet::SCROLL_ADJUST | EffectSet::PHASE_RESET)
    }

    // =========================================================================
    // SELECTION & FOCUS
    // =========================================================================

    fn select(&mut self, node: NodeIdx, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        let chamber = self
            .chamber(world)
            .ok_or(Fault::ChamberNotFound(self.current_chamber))?;

        if node as usize >= chamber.entity_count() {
            return Err(Fault::NodeNotFound(node));
        }

        self.taxonomy.selection = Some(node);
        self.mark_navigation();

        Ok(EffectSet::TAXONOMY_CHANGED | EffectSet::PHASE_RESET)
    }

    fn focus(&mut self, entity: EntityId, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        let chamber = self
            .chamber(world)
            .ok_or(Fault::ChamberNotFound(self.current_chamber))?;

        let idx = chamber
            .find_entity(entity)
            .ok_or(Fault::EntityNotFound(entity))?;

        let pos = chamber.position(idx).ok_or(Fault::EntityNotFound(entity))?;

        self.camera.target = pos;
        self.taxonomy.selection = Some(idx as u32);
        self.mark_navigation();

        Ok(EffectSet::CAMERA_CHANGED
            | EffectSet::TAXONOMY_CHANGED
            | EffectSet::PHASE_RESET
            | EffectSet::PREFETCH_DETAILS)
    }

    fn track(&mut self, entity: EntityId, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        // Same as focus for now - tracking would need animation loop integration
        self.focus(entity, world)
    }

    fn preview(&mut self, node: NodeIdx) -> Result<EffectSet, Fault> {
        self.taxonomy.preview_target = Some(node);
        Ok(EffectSet::PREVIEW_SET)
    }

    fn clear_preview(&mut self) -> Result<EffectSet, Fault> {
        self.taxonomy.preview_target = None;
        Ok(EffectSet::PREVIEW_CLEAR)
    }

    // =========================================================================
    // MODE SWITCHING
    // =========================================================================

    fn mode_spatial(&mut self) -> Result<EffectSet, Fault> {
        if self.mode == NavigationMode::Spatial {
            return Ok(EffectSet::NONE);
        }
        self.mode = NavigationMode::Spatial;
        Ok(EffectSet::MODE_CHANGED)
    }

    fn mode_structural(&mut self) -> Result<EffectSet, Fault> {
        if self.mode == NavigationMode::Structural {
            return Ok(EffectSet::NONE);
        }
        self.mode = NavigationMode::Structural;
        Ok(EffectSet::MODE_CHANGED)
    }

    fn mode_toggle(&mut self) -> Result<EffectSet, Fault> {
        self.mode = match self.mode {
            NavigationMode::Spatial => NavigationMode::Structural,
            NavigationMode::Structural => NavigationMode::Spatial,
        };
        Ok(EffectSet::MODE_CHANGED)
    }

    // =========================================================================
    // HELPERS
    // =========================================================================

    fn mark_navigation(&mut self) {
        self.taxonomy.last_nav_tick = self.tick;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esper_snapshot::{CameraPreset, ChamberKind, Rect, NONE_IDX};

    fn make_test_world() -> WorldSnapshot {
        let chamber = ChamberSnapshot {
            id: 0,
            kind: ChamberKind::Tree,
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            default_camera: CameraPreset::default(),
            entity_ids: vec![100, 101, 102, 103],
            kind_ids: vec![1, 1, 1, 1],
            x: vec![10.0, 20.0, 30.0, 40.0],
            y: vec![10.0, 20.0, 30.0, 40.0],
            label_ids: vec![0, 1, 2, 3],
            detail_refs: vec![100, 101, 102, 103],
            first_child: vec![1, 3, NONE_IDX, NONE_IDX],
            next_sibling: vec![NONE_IDX, 2, NONE_IDX, NONE_IDX],
            prev_sibling: vec![NONE_IDX, NONE_IDX, 1, NONE_IDX],
            doors: vec![],
            grid: esper_snapshot::GridSnapshot::default(),
        };

        WorldSnapshot {
            envelope: esper_snapshot::SnapshotEnvelope {
                schema_version: 1,
                source_hash: 0,
                policy_hash: 0,
                created_at: 0,
                cbu_id: 1,
            },
            string_table: vec![
                "A".to_string(),
                "B".to_string(),
                "C".to_string(),
                "D".to_string(),
            ],
            chambers: vec![chamber],
        }
    }

    #[test]
    fn state_default() {
        let state = DroneState::new();
        assert_eq!(state.mode, NavigationMode::Spatial);
        assert_eq!(state.current_chamber, 0);
        assert!(state.context_stack.is_empty());
    }

    #[test]
    fn state_pan() {
        let world = make_test_world();
        let mut state = DroneState::new();

        let effects = state
            .execute(Verb::PanBy { dx: 10.0, dy: 20.0 }, &world)
            .unwrap();

        assert!(effects.contains(EffectSet::CAMERA_CHANGED));
        assert_eq!(state.camera.target, Vec2::new(10.0, 20.0));
    }

    #[test]
    fn state_zoom() {
        let world = make_test_world();
        let mut state = DroneState::new();

        let effects = state.execute(Verb::Zoom(2.0), &world).unwrap();

        assert!(effects.contains(EffectSet::CAMERA_CHANGED));
        assert_eq!(state.camera.target_zoom, 2.0);
    }

    #[test]
    fn state_next_prev() {
        let world = make_test_world();
        let mut state = DroneState::new();
        state.taxonomy.selection = Some(1); // Entity B

        let effects = state.execute(Verb::Next, &world).unwrap();
        assert!(effects.contains(EffectSet::TAXONOMY_CHANGED));
        assert_eq!(state.taxonomy.selection, Some(2)); // Entity C

        // Can't go next from C (no next sibling)
        let result = state.execute(Verb::Next, &world);
        assert!(matches!(result, Err(Fault::NoNextSibling(_))));

        // Go back
        let _effects = state.execute(Verb::Prev, &world).unwrap();
        assert_eq!(state.taxonomy.selection, Some(1)); // Back to B
    }

    #[test]
    fn state_descend_ascend() {
        let world = make_test_world();
        let mut state = DroneState::new();
        state.taxonomy.selection = Some(0); // Root

        // Descend to first child
        let effects = state.execute(Verb::Descend, &world).unwrap();
        assert!(effects.contains(EffectSet::TAXONOMY_CHANGED));
        assert_eq!(state.taxonomy.selection, Some(1)); // Entity B
        assert_eq!(state.taxonomy.focus_path, vec![0]);

        // Ascend back
        let effects = state.execute(Verb::Ascend, &world).unwrap();
        assert!(effects.contains(EffectSet::TAXONOMY_CHANGED));
        assert_eq!(state.taxonomy.selection, Some(0)); // Back to root
        assert!(state.taxonomy.focus_path.is_empty());
    }

    #[test]
    fn state_focus() {
        let world = make_test_world();
        let mut state = DroneState::new();

        let effects = state.execute(Verb::Focus(102), &world).unwrap();

        assert!(effects.contains(EffectSet::CAMERA_CHANGED));
        assert!(effects.contains(EffectSet::TAXONOMY_CHANGED));
        assert_eq!(state.taxonomy.selection, Some(2)); // Index of entity 102
    }

    #[test]
    fn state_mode_toggle() {
        let world = make_test_world();
        let mut state = DroneState::new();

        assert_eq!(state.mode, NavigationMode::Spatial);

        let effects = state.execute(Verb::ModeToggle, &world).unwrap();
        assert!(effects.contains(EffectSet::MODE_CHANGED));
        assert_eq!(state.mode, NavigationMode::Structural);

        let effects = state.execute(Verb::ModeToggle, &world).unwrap();
        assert!(effects.contains(EffectSet::MODE_CHANGED));
        assert_eq!(state.mode, NavigationMode::Spatial);
    }

    #[test]
    fn state_empty_world() {
        let world = WorldSnapshot::empty(1);
        let mut state = DroneState::new();

        let result = state.execute(Verb::Next, &world);
        assert!(matches!(result, Err(Fault::EmptyWorld)));
    }
}
