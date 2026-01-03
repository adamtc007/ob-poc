//! ViewState - The "it" that session is looking at
//!
//! This module implements the visual state that:
//! - IS what the user sees
//! - IS what operations target
//! - IS what agent knows about
//!
//! Key insight: Session = Intent Scope = Visual State = Operation Target
//!
//! # Fractal Navigation
//!
//! ViewState now uses TaxonomyStack for fractal zoom navigation:
//! - `zoom_in(node_id)` - Push child taxonomy onto stack
//! - `zoom_out()` - Pop back to parent taxonomy
//! - `back_to(index)` - Jump to specific breadcrumb level

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use crate::taxonomy::{Filter, TaxonomyContext, TaxonomyNode, TaxonomyStack};

// =============================================================================
// VIEW STATE - The complete visual state
// =============================================================================

/// View state - the "it" that session is looking at
/// This IS what the user sees, what operations target, what agent knows about
///
/// # Fractal Navigation with TaxonomyStack
///
/// The ViewState now includes a `TaxonomyStack` for fractal zoom navigation:
/// - Each frame on the stack represents a "zoom level"
/// - `zoom_in(node_id)` expands a node into its child taxonomy (pushes frame)
/// - `zoom_out()` returns to the parent taxonomy (pops frame)
/// - `back_to(index)` jumps to a specific breadcrumb level
///
/// The `taxonomy` field always reflects the CURRENT frame's tree.
/// The stack provides the navigation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewState {
    /// Navigation stack for fractal zoom (current view is top of stack)
    #[serde(skip)]
    pub stack: TaxonomyStack,

    /// The taxonomy tree (rebuilt on context change)
    /// NOTE: This is a convenience accessor that mirrors stack.current().tree
    pub taxonomy: TaxonomyNode,

    /// Current context (what built this taxonomy)
    pub context: TaxonomyContext,

    /// Active refinements ("except...", "plus...")
    pub refinements: Vec<Refinement>,

    /// Computed selection (the actual "those" after refinements)
    /// This is what operations target
    pub selection: Vec<Uuid>,

    /// Staged operation awaiting confirmation
    pub pending: Option<PendingOperation>,

    /// Layout result (computed positions for rendering)
    pub layout: Option<LayoutResult>,

    /// When this view was computed
    pub computed_at: DateTime<Utc>,
}

// =============================================================================
// REFINEMENTS - How selection is narrowed/expanded
// =============================================================================

/// Refinement operations that modify the selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Refinement {
    /// Add filter: "only the Luxembourg ones"
    Include { filter: Filter },

    /// Remove filter: "except under 100M"
    Exclude { filter: Filter },

    /// Add specific entities: "and also ABC Fund"
    Add { ids: Vec<Uuid> },

    /// Remove specific entities: "but not that one"
    Remove { ids: Vec<Uuid> },
}

// =============================================================================
// PENDING OPERATIONS - Staged operations awaiting confirmation
// =============================================================================

/// A staged operation awaiting user confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOperation {
    /// What operation
    pub operation: BatchOperation,

    /// Target IDs (from selection)
    pub targets: Vec<Uuid>,

    /// Generated DSL verbs
    pub verbs: String,

    /// Preview of what will happen
    pub preview: OperationPreview,

    /// When staged
    pub staged_at: DateTime<Utc>,
}

/// Batch operations that can be applied to selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchOperation {
    /// Subscribe selection to a product
    Subscribe { product: String },

    /// Unsubscribe selection from a product
    Unsubscribe { product: String },

    /// Set status on selection
    SetStatus { status: String },

    /// Assign role to entity across selection
    AssignRole { entity_id: Uuid, role: String },

    /// Create entities from research findings
    CreateFromResearch,

    /// Enrich existing entities from research
    EnrichFromResearch,

    /// Custom verb with arguments
    Custom {
        verb: String,
        args: HashMap<String, serde_json::Value>,
    },
}

/// Preview of what an operation will do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationPreview {
    /// Summary text: "Add CUSTODY to 12 CBUs"
    pub summary: String,

    /// How many items affected
    pub affected_count: usize,

    /// How many already have it: "3 already have it"
    pub already_done_count: usize,

    /// How many would fail: "2 missing prerequisites"
    pub would_fail_count: usize,

    /// Estimated duration
    #[serde(
        serialize_with = "serialize_duration_opt",
        deserialize_with = "deserialize_duration_opt"
    )]
    pub estimated_duration: Option<Duration>,
}

// =============================================================================
// LAYOUT RESULT - Computed positions for rendering
// =============================================================================

/// Layout result from positioning algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutResult {
    /// Node positions keyed by node ID
    pub positions: HashMap<Uuid, NodePosition>,

    /// Bounds of the layout
    pub bounds: LayoutBounds,

    /// Layout algorithm used
    pub algorithm: String,

    /// When computed
    pub computed_at: DateTime<Utc>,
}

/// Position of a single node
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32,
    pub z: f32, // For 3D/layered views
    pub radius: f32,
}

/// Bounding box of layout
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LayoutBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub min_z: f32,
    pub max_z: f32,
}

// =============================================================================
// VIEW STATE IMPLEMENTATION
// =============================================================================

impl ViewState {
    /// Create empty view state (no taxonomy)
    pub fn empty() -> Self {
        let taxonomy = TaxonomyNode::empty_root();
        Self {
            stack: TaxonomyStack::with_root(taxonomy.clone()),
            taxonomy,
            context: TaxonomyContext::Universe,
            refinements: Vec::new(),
            selection: Vec::new(),
            pending: None,
            layout: None,
            computed_at: Utc::now(),
        }
    }

    /// Create from taxonomy and context
    pub fn from_taxonomy(taxonomy: TaxonomyNode, context: TaxonomyContext) -> Self {
        // Initial selection is all IDs in taxonomy
        let selection = taxonomy.all_ids();

        Self {
            stack: TaxonomyStack::with_root(taxonomy.clone()),
            taxonomy,
            context,
            refinements: Vec::new(),
            selection,
            pending: None,
            layout: None,
            computed_at: Utc::now(),
        }
    }

    /// Apply a refinement, recomputing selection
    pub fn refine(&mut self, refinement: Refinement) {
        // Store the refinement
        self.refinements.push(refinement);

        // Recompute selection from scratch
        self.recompute_selection();
    }

    /// Clear all refinements, restore full selection
    pub fn clear_refinements(&mut self) {
        self.refinements.clear();
        self.selection = self.taxonomy.all_ids();
    }

    /// Recompute selection based on current refinements
    fn recompute_selection(&mut self) {
        // Start with all IDs
        let mut selection: Vec<Uuid> = self.taxonomy.all_ids();

        // Apply each refinement in order
        for refinement in &self.refinements {
            match refinement {
                Refinement::Include { filter } => {
                    // Keep only items matching filter
                    selection.retain(|id| {
                        self.taxonomy
                            .find(*id)
                            .is_some_and(|node| filter.matches(&node.dimensions))
                    });
                }
                Refinement::Exclude { filter } => {
                    // Remove items matching filter
                    selection.retain(|id| {
                        self.taxonomy
                            .find(*id)
                            .is_none_or(|node| !filter.matches(&node.dimensions))
                    });
                }
                Refinement::Add { ids } => {
                    // Add specific IDs (if they exist in taxonomy)
                    for id in ids {
                        if self.taxonomy.find(*id).is_some() && !selection.contains(id) {
                            selection.push(*id);
                        }
                    }
                }
                Refinement::Remove { ids } => {
                    // Remove specific IDs
                    selection.retain(|id| !ids.contains(id));
                }
            }
        }

        self.selection = selection;
    }

    /// Stage an operation for confirmation
    pub fn stage_operation(&mut self, operation: BatchOperation) -> Result<()> {
        if self.selection.is_empty() {
            return Err(anyhow!("No selection to operate on"));
        }

        let targets = self.selection.clone();
        let verbs = self.generate_verbs(&operation, &targets);
        let preview = self.compute_preview(&operation, &targets);

        self.pending = Some(PendingOperation {
            operation,
            targets,
            verbs,
            preview,
            staged_at: Utc::now(),
        });

        Ok(())
    }

    /// Clear pending operation
    pub fn clear_pending(&mut self) {
        self.pending = None;
    }

    /// Get selection count
    pub fn selection_count(&self) -> usize {
        self.selection.len()
    }

    /// Check if has pending operation
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Generate DSL verbs for an operation
    fn generate_verbs(&self, operation: &BatchOperation, targets: &[Uuid]) -> String {
        let mut lines = Vec::new();

        for target in targets {
            let verb = match operation {
                BatchOperation::Subscribe { product } => {
                    format!(
                        "(cbu.add-product :cbu-id \"{}\" :product \"{}\")",
                        target, product
                    )
                }
                BatchOperation::Unsubscribe { product } => {
                    format!(
                        "(cbu.remove-product :cbu-id \"{}\" :product \"{}\")",
                        target, product
                    )
                }
                BatchOperation::SetStatus { status } => {
                    format!("(cbu.update :cbu-id \"{}\" :status \"{}\")", target, status)
                }
                BatchOperation::AssignRole { entity_id, role } => {
                    format!(
                        "(cbu.assign-role :cbu-id \"{}\" :entity-id \"{}\" :role \"{}\")",
                        target, entity_id, role
                    )
                }
                BatchOperation::CreateFromResearch => {
                    format!("(research.execute-create :finding-id \"{}\")", target)
                }
                BatchOperation::EnrichFromResearch => {
                    format!("(research.execute-enrich :finding-id \"{}\")", target)
                }
                BatchOperation::Custom { verb, args } => {
                    let args_str: Vec<String> =
                        args.iter().map(|(k, v)| format!(":{} {}", k, v)).collect();
                    format!(
                        "({} :target-id \"{}\" {})",
                        verb,
                        target,
                        args_str.join(" ")
                    )
                }
            };
            lines.push(verb);
        }

        lines.join("\n")
    }

    /// Compute preview for an operation
    fn compute_preview(&self, operation: &BatchOperation, targets: &[Uuid]) -> OperationPreview {
        let summary = match operation {
            BatchOperation::Subscribe { product } => {
                format!("Add {} to {} CBUs", product, targets.len())
            }
            BatchOperation::Unsubscribe { product } => {
                format!("Remove {} from {} CBUs", product, targets.len())
            }
            BatchOperation::SetStatus { status } => {
                format!("Set status to {} for {} items", status, targets.len())
            }
            BatchOperation::AssignRole { role, .. } => {
                format!("Assign {} role to {} CBUs", role, targets.len())
            }
            BatchOperation::CreateFromResearch => {
                format!("Create {} entities from research", targets.len())
            }
            BatchOperation::EnrichFromResearch => {
                format!("Enrich {} entities from research", targets.len())
            }
            BatchOperation::Custom { verb, .. } => {
                format!("Execute {} on {} items", verb, targets.len())
            }
        };

        // TODO: These would be computed from actual database state
        OperationPreview {
            summary,
            affected_count: targets.len(),
            already_done_count: 0,
            would_fail_count: 0,
            estimated_duration: Some(Duration::from_millis(targets.len() as u64 * 50)),
        }
    }

    /// Get taxonomy metaphor
    pub fn metaphor(&self) -> crate::taxonomy::Metaphor {
        self.taxonomy.metaphor()
    }

    /// Get taxonomy astro level
    pub fn astro_level(&self) -> crate::taxonomy::AstroLevel {
        self.taxonomy.astro_level()
    }

    // =========================================================================
    // FRACTAL NAVIGATION - Zoom in/out via TaxonomyStack
    // =========================================================================

    /// Zoom into a node, expanding it into its child taxonomy.
    ///
    /// If the node has an ExpansionRule::Parser, the parser is invoked
    /// to build the child taxonomy and push it onto the stack.
    ///
    /// Returns Ok(true) if zoom succeeded, Ok(false) if node not expandable.
    pub async fn zoom_in(&mut self, node_id: Uuid) -> Result<bool> {
        use crate::taxonomy::{ExpansionRule, TaxonomyFrame};

        // Find the node in current taxonomy
        let node = self
            .taxonomy
            .find(node_id)
            .ok_or_else(|| anyhow!("Node {} not found in current taxonomy", node_id))?;

        // Check expansion rule
        match &node.expansion {
            ExpansionRule::Parser(parser) => {
                // Parse child taxonomy
                let child_tree = parser.parse_for(node_id).await.map_err(|e| {
                    anyhow!("Failed to parse child taxonomy for {}: {}", node_id, e)
                })?;

                // Create new frame using from_zoom
                let frame = TaxonomyFrame::from_zoom(
                    node_id,
                    &node.label,
                    child_tree.clone(),
                    Some(parser.clone_arc()),
                );

                // Push onto stack (ignore max depth error, just return false)
                if self.stack.push(frame).is_err() {
                    return Ok(false);
                }

                // Update convenience accessor
                self.taxonomy = child_tree.clone();
                self.selection = child_tree.all_ids();
                self.refinements.clear();
                self.layout = None;
                self.computed_at = Utc::now();

                Ok(true)
            }
            ExpansionRule::Context(ctx) => {
                // Context-based expansion - would need a builder
                // For now, return false (not directly expandable)
                tracing::debug!(
                    "Node {} has Context expansion, not directly expandable: {:?}",
                    node_id,
                    ctx
                );
                Ok(false)
            }
            ExpansionRule::Complete | ExpansionRule::Terminal => {
                // Not expandable
                Ok(false)
            }
        }
    }

    /// Zoom out to the parent taxonomy.
    ///
    /// Pops the current frame from the stack and restores the parent view.
    /// Returns Ok(true) if zoom out succeeded, Ok(false) if already at root.
    pub fn zoom_out(&mut self) -> Result<bool> {
        if self.stack.depth() <= 1 {
            // Already at root, can't zoom out further
            return Ok(false);
        }

        // Pop current frame
        self.stack.pop();

        // Update from new current frame
        if let Some(frame) = self.stack.current() {
            self.taxonomy = frame.tree.clone();
            self.selection = frame.selection.clone();
            self.refinements.clear();
            self.layout = None;
            self.computed_at = Utc::now();
        }

        Ok(true)
    }

    /// Jump back to a specific breadcrumb level.
    ///
    /// `depth` is 0-indexed: 0 = root, 1 = first zoom, etc.
    /// Returns Ok(true) if jump succeeded, Ok(false) if invalid depth.
    pub fn back_to(&mut self, depth: usize) -> Result<bool> {
        if depth >= self.stack.depth() {
            return Ok(false);
        }

        // Pop down to target depth
        self.stack.pop_to_depth(depth + 1); // +1 because depth is 0-indexed but we want to keep that frame

        // Update from new current frame
        if let Some(frame) = self.stack.current() {
            self.taxonomy = frame.tree.clone();
            self.selection = frame.selection.clone();
            self.refinements.clear();
            self.layout = None;
            self.computed_at = Utc::now();
        }

        Ok(true)
    }

    /// Get breadcrumbs for navigation display.
    ///
    /// Returns a list of labels from root to current.
    pub fn breadcrumbs(&self) -> Vec<String> {
        self.stack.breadcrumbs()
    }

    /// Get breadcrumbs with frame IDs for navigation.
    ///
    /// Returns a list of (label, frame_id) pairs from root to current.
    pub fn breadcrumbs_with_ids(&self) -> Vec<(String, Uuid)> {
        self.stack
            .frames()
            .iter()
            .map(|f| (f.label.clone(), f.frame_id))
            .collect()
    }

    /// Get current zoom depth (0 = root level).
    pub fn zoom_depth(&self) -> usize {
        self.stack.depth().saturating_sub(1)
    }

    /// Check if we can zoom out (not at root).
    pub fn can_zoom_out(&self) -> bool {
        self.stack.depth() > 1
    }

    /// Check if a node can be zoomed into.
    pub fn can_zoom_in(&self, node_id: Uuid) -> bool {
        use crate::taxonomy::ExpansionRule;

        self.taxonomy
            .find(node_id)
            .is_some_and(|node| matches!(node.expansion, ExpansionRule::Parser(_)))
    }

    /// Sync taxonomy field from stack (call after stack modifications).
    fn sync_from_stack(&mut self) {
        if let Some(frame) = self.stack.current() {
            self.taxonomy = frame.tree.clone();
            self.selection = frame.selection.clone();
        }
    }
}

impl LayoutBounds {
    /// Create default bounds
    pub fn default_bounds() -> Self {
        Self {
            min_x: -1000.0,
            max_x: 1000.0,
            min_y: -1000.0,
            max_y: 1000.0,
            min_z: 0.0,
            max_z: 100.0,
        }
    }

    /// Width of bounds
    pub fn width(&self) -> f32 {
        self.max_x - self.min_x
    }

    /// Height of bounds
    pub fn height(&self) -> f32 {
        self.max_y - self.min_y
    }

    /// Depth of bounds
    pub fn depth(&self) -> f32 {
        self.max_z - self.min_z
    }
}

// =============================================================================
// SERDE HELPERS FOR DURATION
// =============================================================================

fn serialize_duration_opt<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match duration {
        Some(d) => serializer.serialize_some(&d.as_millis()),
        None => serializer.serialize_none(),
    }
}

fn deserialize_duration_opt<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<u64> = Option::deserialize(deserializer)?;
    Ok(opt.map(Duration::from_millis))
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taxonomy::{DimensionValues, NodeType, Status};

    fn make_test_taxonomy() -> TaxonomyNode {
        // Create a small test taxonomy
        let mut root = TaxonomyNode::new(
            Uuid::new_v4(),
            NodeType::Root,
            "Universe".to_string(),
            DimensionValues::default(),
        );

        // Add some children with different dimensions
        for i in 0..5 {
            let jurisdiction = if i % 2 == 0 { "LU" } else { "IE" };
            let status = if i % 3 == 0 {
                Status::Green
            } else {
                Status::Amber
            };

            let dims = DimensionValues {
                jurisdiction: Some(jurisdiction.to_string()),
                status: Some(status),
                ..Default::default()
            };

            let child =
                TaxonomyNode::new(Uuid::new_v4(), NodeType::Cbu, format!("CBU {}", i), dims);
            root.children.push(child);
        }

        root.compute_metrics();
        root
    }

    #[test]
    fn test_view_state_creation() {
        let taxonomy = make_test_taxonomy();
        let view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        // Should have all IDs in selection (root + 5 children = 6)
        assert_eq!(view.selection.len(), 6);
        assert!(view.refinements.is_empty());
        assert!(view.pending.is_none());
    }

    #[test]
    fn test_refinement_include() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        let original_count = view.selection.len();

        // Include only Luxembourg
        view.refine(Refinement::Include {
            filter: Filter::Jurisdiction(vec!["LU".to_string()]),
        });

        // Should have fewer items
        assert!(view.selection.len() < original_count);
    }

    #[test]
    fn test_refinement_exclude() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        let original_count = view.selection.len();

        // Exclude green status
        view.refine(Refinement::Exclude {
            filter: Filter::Status(vec![Status::Green]),
        });

        // Should have fewer items
        assert!(view.selection.len() < original_count);
    }

    #[test]
    fn test_clear_refinements() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        let original_count = view.selection.len();

        // Add some refinements
        view.refine(Refinement::Include {
            filter: Filter::Jurisdiction(vec!["LU".to_string()]),
        });
        assert!(view.selection.len() < original_count);

        // Clear
        view.clear_refinements();
        assert_eq!(view.selection.len(), original_count);
    }

    #[test]
    fn test_stage_operation() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        // Stage an operation
        view.stage_operation(BatchOperation::Subscribe {
            product: "CUSTODY".to_string(),
        })
        .unwrap();

        assert!(view.has_pending());
        let pending = view.pending.as_ref().unwrap();
        assert_eq!(pending.targets.len(), view.selection.len());
        assert!(pending.verbs.contains("cbu.add-product"));
    }

    #[test]
    fn test_stage_operation_empty_selection() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        // Clear selection
        view.selection.clear();

        // Should fail
        let result = view.stage_operation(BatchOperation::Subscribe {
            product: "CUSTODY".to_string(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_add_remove_refinements() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        let new_id = Uuid::new_v4();

        // Remove an existing ID
        let first_id = view.selection[0];
        view.refine(Refinement::Remove {
            ids: vec![first_id],
        });
        assert!(!view.selection.contains(&first_id));

        // Try to add a non-existent ID (should not be added)
        let before_count = view.selection.len();
        view.refine(Refinement::Add { ids: vec![new_id] });
        assert_eq!(view.selection.len(), before_count); // unchanged
    }
}
