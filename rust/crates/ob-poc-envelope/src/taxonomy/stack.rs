//! TaxonomyStack - Stack-based fractal navigation
//!
//! The stack enables zoom-in/zoom-out navigation through taxonomies.
//! Each frame represents a level of zoom, with the bottom being the universe
//! and each push zooming into a child taxonomy.
//!
//! # Example Navigation
//!
//! ```text
//! Initial:  [Universe]
//! Zoom in:  [Universe, CBU "Apex Fund"]
//! Zoom in:  [Universe, CBU "Apex Fund", Trading Matrix]
//! Zoom out: [Universe, CBU "Apex Fund"]
//! Back to:  [Universe]
//! ```
//!
//! # Key Operations
//!
//! - `zoom_in(node_id)` - Push a new frame for the node's expansion
//! - `zoom_out()` - Pop the top frame
//! - `back_to(frame_index)` - Pop frames until reaching the target
//! - `current()` - Get the current (top) taxonomy tree

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::combinators::TaxonomyParser;
use super::node::TaxonomyNode;
use super::rules::TaxonomyContext;
use super::types::Filter;

// =============================================================================
// TAXONOMY FRAME - A single level in the navigation stack
// =============================================================================

/// A frame in the taxonomy navigation stack.
/// Each frame represents a zoom level with its own taxonomy tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyFrame {
    /// Unique identifier for this frame
    pub frame_id: Uuid,

    /// The node that was zoomed into (None for root frame)
    pub focus_node_id: Option<Uuid>,

    /// Label for breadcrumb display
    pub label: String,

    /// Short label for compact breadcrumb
    pub short_label: Option<String>,

    /// The taxonomy tree for this frame
    pub tree: TaxonomyNode,

    /// Context that generated this frame
    #[serde(skip)]
    pub context: Option<TaxonomyContext>,

    /// Parser used to generate this frame (for refresh)
    #[serde(skip)]
    pub parser: Option<Arc<dyn TaxonomyParser + Send + Sync>>,

    /// Active filters applied to this frame
    pub filters: Vec<Filter>,

    /// Selected node IDs in this frame
    pub selection: Vec<Uuid>,

    /// Whether this frame is fully loaded
    pub is_loaded: bool,

    /// Whether this frame is currently loading
    pub is_loading: bool,
}

impl TaxonomyFrame {
    /// Create a new root frame
    pub fn root(tree: TaxonomyNode) -> Self {
        Self {
            frame_id: Uuid::new_v4(),
            focus_node_id: None,
            label: tree.label.clone(),
            short_label: tree.short_label.clone(),
            tree,
            context: None,
            parser: None,
            filters: Vec::new(),
            selection: Vec::new(),
            is_loaded: true,
            is_loading: false,
        }
    }

    /// Create a frame from zooming into a node
    pub fn from_zoom(
        focus_node_id: Uuid,
        label: impl Into<String>,
        tree: TaxonomyNode,
        parser: Option<Arc<dyn TaxonomyParser + Send + Sync>>,
    ) -> Self {
        Self {
            frame_id: Uuid::new_v4(),
            focus_node_id: Some(focus_node_id),
            label: label.into(),
            short_label: None,
            tree,
            context: None,
            parser,
            filters: Vec::new(),
            selection: Vec::new(),
            is_loaded: true,
            is_loading: false,
        }
    }

    /// Create a loading placeholder frame
    pub fn loading(focus_node_id: Uuid, label: impl Into<String>) -> Self {
        Self {
            frame_id: Uuid::new_v4(),
            focus_node_id: Some(focus_node_id),
            label: label.into(),
            short_label: None,
            tree: TaxonomyNode::empty_root(),
            context: None,
            parser: None,
            filters: Vec::new(),
            selection: Vec::new(),
            is_loaded: false,
            is_loading: true,
        }
    }

    /// Apply a filter to this frame
    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filters.push(filter);
        self
    }

    /// Get matching node IDs based on current filters
    pub fn filtered_ids(&self) -> Vec<Uuid> {
        if self.filters.is_empty() {
            self.tree.all_ids()
        } else {
            let mut ids = self.tree.all_ids();
            for filter in &self.filters {
                let matching = self.tree.matching_ids(filter);
                ids.retain(|id| matching.contains(id));
            }
            ids
        }
    }

    /// Check if a node passes all filters
    pub fn passes_filters(&self, node: &TaxonomyNode) -> bool {
        self.filters.iter().all(|f| f.matches(&node.dimensions))
    }
}

// =============================================================================
// TAXONOMY STACK - The navigation stack
// =============================================================================

/// Stack-based navigation through taxonomy hierarchies.
/// Supports zoom-in, zoom-out, and back-to operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyStack {
    /// The stack of frames (bottom = root, top = current view)
    frames: Vec<TaxonomyFrame>,

    /// Maximum stack depth (prevents infinite zoom)
    max_depth: usize,
}

impl Default for TaxonomyStack {
    fn default() -> Self {
        Self::new()
    }
}

impl TaxonomyStack {
    /// Create an empty stack
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            max_depth: 10,
        }
    }

    /// Create a stack with a root frame
    pub fn with_root(tree: TaxonomyNode) -> Self {
        Self {
            frames: vec![TaxonomyFrame::root(tree)],
            max_depth: 10,
        }
    }

    /// Set maximum depth
    pub fn with_max_depth(mut self, max: usize) -> Self {
        self.max_depth = max;
        self
    }

    // =========================================================================
    // ACCESSORS
    // =========================================================================

    /// Get the current (top) frame
    pub fn current(&self) -> Option<&TaxonomyFrame> {
        self.frames.last()
    }

    /// Get the current frame mutably
    pub fn current_mut(&mut self) -> Option<&mut TaxonomyFrame> {
        self.frames.last_mut()
    }

    /// Get the current taxonomy tree
    pub fn current_tree(&self) -> Option<&TaxonomyNode> {
        self.current().map(|f| &f.tree)
    }

    /// Get the root frame
    pub fn root(&self) -> Option<&TaxonomyFrame> {
        self.frames.first()
    }

    /// Get all frames (for breadcrumb display)
    pub fn frames(&self) -> &[TaxonomyFrame] {
        &self.frames
    }

    /// Get breadcrumb labels
    pub fn breadcrumbs(&self) -> Vec<String> {
        self.frames
            .iter()
            .map(|f| f.short_label.clone().unwrap_or_else(|| f.label.clone()))
            .collect()
    }

    /// Get breadcrumbs with both display label and type code
    /// Returns Vec<(display_label, type_code)> where type_code is the full label
    pub fn breadcrumbs_with_codes(&self) -> Vec<(String, String)> {
        self.frames
            .iter()
            .map(|f| {
                let display = f.short_label.clone().unwrap_or_else(|| f.label.clone());
                let code = f.label.clone(); // Use full label as type code
                (display, code)
            })
            .collect()
    }

    /// Current depth (0 = empty, 1 = root only)
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Is the stack empty?
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Is at root level?
    pub fn is_at_root(&self) -> bool {
        self.frames.len() == 1
    }

    /// Can zoom out?
    pub fn can_zoom_out(&self) -> bool {
        self.frames.len() > 1
    }

    /// Can zoom in? (not at max depth)
    pub fn can_zoom_in(&self) -> bool {
        self.frames.len() < self.max_depth
    }

    // =========================================================================
    // NAVIGATION OPERATIONS
    // =========================================================================

    /// Push a new frame onto the stack (zoom in)
    pub fn push(&mut self, frame: TaxonomyFrame) -> Result<(), StackError> {
        if self.frames.len() >= self.max_depth {
            return Err(StackError::MaxDepthReached(self.max_depth));
        }
        self.frames.push(frame);
        Ok(())
    }

    /// Pop the top frame (zoom out)
    pub fn pop(&mut self) -> Option<TaxonomyFrame> {
        if self.frames.len() > 1 {
            self.frames.pop()
        } else {
            None // Cannot pop root
        }
    }

    /// Pop frames until reaching target depth
    pub fn pop_to_depth(&mut self, target_depth: usize) -> Vec<TaxonomyFrame> {
        let mut popped = Vec::new();
        while self.frames.len() > target_depth && self.frames.len() > 1 {
            if let Some(frame) = self.frames.pop() {
                popped.push(frame);
            }
        }
        popped
    }

    /// Pop frames until reaching a specific frame
    pub fn pop_to_frame(&mut self, frame_id: Uuid) -> Vec<TaxonomyFrame> {
        let target_idx = self.frames.iter().position(|f| f.frame_id == frame_id);
        if let Some(idx) = target_idx {
            self.pop_to_depth(idx + 1)
        } else {
            Vec::new()
        }
    }

    /// Replace the current frame's tree (for refresh/update)
    pub fn update_current_tree(&mut self, tree: TaxonomyNode) {
        if let Some(frame) = self.current_mut() {
            frame.tree = tree;
            frame.is_loaded = true;
            frame.is_loading = false;
        }
    }

    /// Set the root frame
    pub fn set_root(&mut self, tree: TaxonomyNode) {
        self.frames.clear();
        self.frames.push(TaxonomyFrame::root(tree));
    }

    /// Clear the entire stack
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    // =========================================================================
    // SELECTION OPERATIONS
    // =========================================================================

    /// Select a node in the current frame
    pub fn select(&mut self, node_id: Uuid) {
        if let Some(frame) = self.current_mut() {
            if !frame.selection.contains(&node_id) {
                frame.selection.push(node_id);
            }
        }
    }

    /// Deselect a node in the current frame
    pub fn deselect(&mut self, node_id: Uuid) {
        if let Some(frame) = self.current_mut() {
            frame.selection.retain(|id| *id != node_id);
        }
    }

    /// Toggle selection
    pub fn toggle_selection(&mut self, node_id: Uuid) {
        if let Some(frame) = self.current_mut() {
            if frame.selection.contains(&node_id) {
                frame.selection.retain(|id| *id != node_id);
            } else {
                frame.selection.push(node_id);
            }
        }
    }

    /// Clear selection in current frame
    pub fn clear_selection(&mut self) {
        if let Some(frame) = self.current_mut() {
            frame.selection.clear();
        }
    }

    /// Get current selection
    pub fn selection(&self) -> &[Uuid] {
        self.current()
            .map(|f| f.selection.as_slice())
            .unwrap_or(&[])
    }

    // =========================================================================
    // FILTER OPERATIONS
    // =========================================================================

    /// Add filter to current frame
    pub fn add_filter(&mut self, filter: Filter) {
        if let Some(frame) = self.current_mut() {
            frame.filters.push(filter);
        }
    }

    /// Remove filter from current frame
    pub fn remove_filter(&mut self, index: usize) {
        if let Some(frame) = self.current_mut() {
            if index < frame.filters.len() {
                frame.filters.remove(index);
            }
        }
    }

    /// Clear filters in current frame
    pub fn clear_filters(&mut self) {
        if let Some(frame) = self.current_mut() {
            frame.filters.clear();
        }
    }

    /// Get current filters
    pub fn filters(&self) -> &[Filter] {
        self.current().map(|f| f.filters.as_slice()).unwrap_or(&[])
    }
}

// =============================================================================
// ERRORS
// =============================================================================

/// Stack operation errors
#[derive(Debug, thiserror::Error)]
pub enum StackError {
    #[error("Maximum stack depth ({0}) reached")]
    MaxDepthReached(usize),

    #[error("Cannot pop root frame")]
    CannotPopRoot,

    #[error("Frame not found: {0}")]
    FrameNotFound(Uuid),

    #[error("Node not found: {0}")]
    NodeNotFound(Uuid),

    #[error("Node is not expandable")]
    NotExpandable,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taxonomy::types::DimensionValues;

    fn make_test_tree() -> TaxonomyNode {
        let mut root = TaxonomyNode::root("Universe");
        for i in 0..3 {
            let child = TaxonomyNode::new(
                Uuid::new_v4(),
                crate::taxonomy::types::NodeType::Cbu,
                format!("CBU {}", i),
                DimensionValues::default(),
            );
            root.add_child(child);
        }
        root.compute_metrics();
        root
    }

    #[test]
    fn test_stack_basic() {
        let tree = make_test_tree();
        let stack = TaxonomyStack::with_root(tree);

        assert_eq!(stack.depth(), 1);
        assert!(stack.is_at_root());
        assert!(!stack.can_zoom_out());
        assert!(stack.can_zoom_in());
    }

    #[test]
    fn test_stack_zoom() {
        let tree = make_test_tree();
        let mut stack = TaxonomyStack::with_root(tree.clone());

        // Zoom in
        let child_tree = TaxonomyNode::root("Child Level");
        let frame = TaxonomyFrame::from_zoom(Uuid::new_v4(), "Level 2", child_tree, None);
        stack.push(frame).unwrap();

        assert_eq!(stack.depth(), 2);
        assert!(!stack.is_at_root());
        assert!(stack.can_zoom_out());

        // Zoom out
        let popped = stack.pop();
        assert!(popped.is_some());
        assert_eq!(stack.depth(), 1);
        assert!(stack.is_at_root());
    }

    #[test]
    fn test_breadcrumbs() {
        let tree = make_test_tree();
        let mut stack = TaxonomyStack::with_root(tree);

        stack
            .push(TaxonomyFrame::from_zoom(
                Uuid::new_v4(),
                "CBU Alpha",
                TaxonomyNode::root("Alpha"),
                None,
            ))
            .unwrap();

        stack
            .push(TaxonomyFrame::from_zoom(
                Uuid::new_v4(),
                "Trading Matrix",
                TaxonomyNode::root("Trading"),
                None,
            ))
            .unwrap();

        let crumbs = stack.breadcrumbs();
        assert_eq!(crumbs, vec!["Universe", "CBU Alpha", "Trading Matrix"]);
    }

    #[test]
    fn test_pop_to_depth() {
        let tree = make_test_tree();
        let mut stack = TaxonomyStack::with_root(tree);

        // Add 3 more levels
        for i in 1..=3 {
            stack
                .push(TaxonomyFrame::from_zoom(
                    Uuid::new_v4(),
                    format!("Level {}", i),
                    TaxonomyNode::root(format!("L{}", i)),
                    None,
                ))
                .unwrap();
        }

        assert_eq!(stack.depth(), 4);

        // Pop to depth 2
        let popped = stack.pop_to_depth(2);
        assert_eq!(popped.len(), 2);
        assert_eq!(stack.depth(), 2);
    }

    #[test]
    fn test_selection() {
        let tree = make_test_tree();
        let mut stack = TaxonomyStack::with_root(tree);

        let node_id = Uuid::new_v4();
        stack.select(node_id);
        assert!(stack.selection().contains(&node_id));

        stack.deselect(node_id);
        assert!(!stack.selection().contains(&node_id));

        stack.toggle_selection(node_id);
        assert!(stack.selection().contains(&node_id));

        stack.toggle_selection(node_id);
        assert!(!stack.selection().contains(&node_id));
    }

    #[test]
    fn test_max_depth() {
        let tree = make_test_tree();
        let mut stack = TaxonomyStack::with_root(tree).with_max_depth(3);

        // Can push 2 more (already have 1)
        stack
            .push(TaxonomyFrame::from_zoom(
                Uuid::new_v4(),
                "L1",
                TaxonomyNode::root("L1"),
                None,
            ))
            .unwrap();
        stack
            .push(TaxonomyFrame::from_zoom(
                Uuid::new_v4(),
                "L2",
                TaxonomyNode::root("L2"),
                None,
            ))
            .unwrap();

        // Should fail
        let result = stack.push(TaxonomyFrame::from_zoom(
            Uuid::new_v4(),
            "L3",
            TaxonomyNode::root("L3"),
            None,
        ));
        assert!(matches!(result, Err(StackError::MaxDepthReached(3))));
    }
}
