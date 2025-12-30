//! Navigation Command Types
//!
//! This module defines the NavCommand enum representing all possible navigation
//! commands that can be issued against an EntityGraph. Commands are grouped into:
//!
//! - **Scope commands**: Change what data is loaded (CBU, Book, Jurisdiction)
//! - **Filter commands**: Change visibility without changing data
//! - **Navigation commands**: Move the cursor within the graph
//! - **Query commands**: Find entities or information
//! - **Display commands**: Change visualization state
//!
//! ## Design Principles
//!
//! Commands are designed to be:
//! 1. **Parseable from natural language** - "show me the Allianz book"
//! 2. **Composable** - Multiple commands can be executed in sequence
//! 3. **Reversible** - Navigation commands support back/forward

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::graph::ProngFilter;

// =============================================================================
// NAVIGATION COMMAND ENUM
// =============================================================================

/// A navigation command to execute against an EntityGraph
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NavCommand {
    // =========================================================================
    // SCOPE COMMANDS - Change what data is loaded
    // =========================================================================
    /// Load a single CBU by name
    LoadCbu { cbu_name: String },

    /// Load all CBUs under a commercial client (book view)
    LoadBook { client_name: String },

    /// Load all entities in a jurisdiction
    LoadJurisdiction { code: String },

    /// Load neighborhood around an entity
    LoadNeighborhood { entity_name: String, hops: u32 },

    // =========================================================================
    // FILTER COMMANDS - Change visibility without reloading
    // =========================================================================
    /// Filter to specific jurisdiction(s)
    FilterJurisdiction { codes: Vec<String> },

    /// Filter to specific fund type(s)
    FilterFundType { fund_types: Vec<String> },

    /// Filter by prong (ownership, control, or both)
    FilterProng { prong: ProngFilter },

    /// Set minimum ownership percentage threshold
    FilterMinOwnership { percentage: f64 },

    /// Filter to show only path from cursor to terminus
    FilterPathOnly { enabled: bool },

    /// Clear all filters
    ClearFilters,

    /// Set temporal filter (as-of date)
    AsOfDate { date: NaiveDate },

    // =========================================================================
    // NAVIGATION COMMANDS - Move cursor within graph
    // =========================================================================
    /// Navigate to a specific entity by name
    GoTo { entity_name: String },

    /// Navigate up the ownership chain (to owner)
    GoUp,

    /// Navigate down the ownership chain (to owned)
    /// If multiple children, can specify by index or name
    GoDown {
        index: Option<usize>,
        name: Option<String>,
    },

    /// Navigate to sibling (same level in ownership tree)
    GoSibling { direction: Direction },

    /// Navigate to ownership terminus (top of chain)
    GoToTerminus,

    /// Navigate to commercial client (book apex)
    GoToClient,

    /// Go back in navigation history
    GoBack,

    /// Go forward in navigation history
    GoForward,

    // =========================================================================
    // QUERY COMMANDS - Find information without navigating
    // =========================================================================
    /// Find entities matching a name pattern
    Find { name_pattern: String },

    /// Find where a person appears with optional role filter
    WhereIs {
        person_name: String,
        role: Option<String>,
    },

    /// Find all entities with a specific role
    FindByRole { role: String },

    /// List children (owned entities) of current node
    ListChildren,

    /// List owners of current node
    ListOwners,

    /// List controllers of current node
    ListControllers,

    /// List all CBUs in current scope
    ListCbus,

    // =========================================================================
    // DISPLAY COMMANDS - Change visualization state
    // =========================================================================
    /// Show path from current node to terminus
    ShowPath,

    /// Show context information about current node
    ShowContext,

    /// Show tree rooted at current node to specified depth
    ShowTree { depth: u32 },

    /// Expand a CBU container to show its entities
    ExpandCbu { cbu_name: Option<String> },

    /// Collapse a CBU container
    CollapseCbu { cbu_name: Option<String> },

    /// Change zoom level
    Zoom { level: ZoomLevel },

    /// Zoom in one step
    ZoomIn,

    /// Zoom out one step
    ZoomOut,

    /// Fit entire graph in view
    FitToView,

    // =========================================================================
    // META COMMANDS
    // =========================================================================
    /// Show help for navigation commands
    Help { topic: Option<String> },

    /// Undo last command
    Undo,

    /// Redo undone command
    Redo,
}

// =============================================================================
// SUPPORTING TYPES
// =============================================================================

/// Direction for sibling navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Previous sibling (left in horizontal layout)
    Left,
    /// Next sibling (right in horizontal layout)
    Right,
    /// Next in iteration order
    Next,
    /// Previous in iteration order
    Prev,
}

/// Zoom level for graph display
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ZoomLevel {
    /// Overview - see entire graph structure
    Overview,
    /// Standard - default zoom level
    #[default]
    Standard,
    /// Detail - close-up on individual nodes
    Detail,
    /// Custom zoom factor (1.0 = 100%)
    Custom(f32),
}

impl ZoomLevel {
    /// Get the zoom factor as a multiplier
    pub fn factor(&self) -> f32 {
        match self {
            ZoomLevel::Overview => 0.25,
            ZoomLevel::Standard => 1.0,
            ZoomLevel::Detail => 2.0,
            ZoomLevel::Custom(f) => *f,
        }
    }

    /// Zoom in one step
    pub fn zoom_in(&self) -> ZoomLevel {
        let current = self.factor();
        ZoomLevel::Custom((current * 1.25).min(4.0))
    }

    /// Zoom out one step
    pub fn zoom_out(&self) -> ZoomLevel {
        let current = self.factor();
        ZoomLevel::Custom((current / 1.25).max(0.1))
    }
}

// =============================================================================
// COMMAND CATEGORIES (for help/documentation)
// =============================================================================

/// Category of navigation command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Scope,
    Filter,
    Navigation,
    Query,
    Display,
    Meta,
}

impl NavCommand {
    /// Get the category of this command
    pub fn category(&self) -> CommandCategory {
        match self {
            NavCommand::LoadCbu { .. }
            | NavCommand::LoadBook { .. }
            | NavCommand::LoadJurisdiction { .. }
            | NavCommand::LoadNeighborhood { .. } => CommandCategory::Scope,

            NavCommand::FilterJurisdiction { .. }
            | NavCommand::FilterFundType { .. }
            | NavCommand::FilterProng { .. }
            | NavCommand::FilterMinOwnership { .. }
            | NavCommand::FilterPathOnly { .. }
            | NavCommand::ClearFilters
            | NavCommand::AsOfDate { .. } => CommandCategory::Filter,

            NavCommand::GoTo { .. }
            | NavCommand::GoUp
            | NavCommand::GoDown { .. }
            | NavCommand::GoSibling { .. }
            | NavCommand::GoToTerminus
            | NavCommand::GoToClient
            | NavCommand::GoBack
            | NavCommand::GoForward => CommandCategory::Navigation,

            NavCommand::Find { .. }
            | NavCommand::WhereIs { .. }
            | NavCommand::FindByRole { .. }
            | NavCommand::ListChildren
            | NavCommand::ListOwners
            | NavCommand::ListControllers
            | NavCommand::ListCbus => CommandCategory::Query,

            NavCommand::ShowPath
            | NavCommand::ShowContext
            | NavCommand::ShowTree { .. }
            | NavCommand::ExpandCbu { .. }
            | NavCommand::CollapseCbu { .. }
            | NavCommand::Zoom { .. }
            | NavCommand::ZoomIn
            | NavCommand::ZoomOut
            | NavCommand::FitToView => CommandCategory::Display,

            NavCommand::Help { .. } | NavCommand::Undo | NavCommand::Redo => CommandCategory::Meta,
        }
    }

    /// Get a human-readable description of this command
    pub fn description(&self) -> &'static str {
        match self {
            NavCommand::LoadCbu { .. } => "Load a single CBU by name",
            NavCommand::LoadBook { .. } => "Load all CBUs under a commercial client",
            NavCommand::LoadJurisdiction { .. } => "Load all entities in a jurisdiction",
            NavCommand::LoadNeighborhood { .. } => "Load entities around a specific entity",
            NavCommand::FilterJurisdiction { .. } => "Filter to specific jurisdiction(s)",
            NavCommand::FilterFundType { .. } => "Filter to specific fund type(s)",
            NavCommand::FilterProng { .. } => "Filter by ownership/control prong",
            NavCommand::FilterMinOwnership { .. } => "Set minimum ownership percentage",
            NavCommand::FilterPathOnly { .. } => "Show only path to terminus",
            NavCommand::ClearFilters => "Clear all active filters",
            NavCommand::AsOfDate { .. } => "Set temporal as-of date",
            NavCommand::GoTo { .. } => "Navigate to a specific entity",
            NavCommand::GoUp => "Navigate to owner/parent",
            NavCommand::GoDown { .. } => "Navigate to owned/child entity",
            NavCommand::GoSibling { .. } => "Navigate to sibling entity",
            NavCommand::GoToTerminus => "Navigate to ownership terminus",
            NavCommand::GoToClient => "Navigate to commercial client",
            NavCommand::GoBack => "Go back in navigation history",
            NavCommand::GoForward => "Go forward in navigation history",
            NavCommand::Find { .. } => "Find entities by name",
            NavCommand::WhereIs { .. } => "Find where a person has roles",
            NavCommand::FindByRole { .. } => "Find entities with a specific role",
            NavCommand::ListChildren => "List owned entities",
            NavCommand::ListOwners => "List owning entities",
            NavCommand::ListControllers => "List controlling entities",
            NavCommand::ListCbus => "List all CBUs in scope",
            NavCommand::ShowPath => "Show path to terminus",
            NavCommand::ShowContext => "Show node context info",
            NavCommand::ShowTree { .. } => "Show tree from current node",
            NavCommand::ExpandCbu { .. } => "Expand CBU container",
            NavCommand::CollapseCbu { .. } => "Collapse CBU container",
            NavCommand::Zoom { .. } => "Set zoom level",
            NavCommand::ZoomIn => "Zoom in",
            NavCommand::ZoomOut => "Zoom out",
            NavCommand::FitToView => "Fit graph to view",
            NavCommand::Help { .. } => "Show help",
            NavCommand::Undo => "Undo last command",
            NavCommand::Redo => "Redo undone command",
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_categories() {
        assert_eq!(
            NavCommand::LoadCbu {
                cbu_name: "Test".to_string()
            }
            .category(),
            CommandCategory::Scope
        );
        assert_eq!(NavCommand::GoUp.category(), CommandCategory::Navigation);
        assert_eq!(
            NavCommand::Find {
                name_pattern: "X".to_string()
            }
            .category(),
            CommandCategory::Query
        );
        assert_eq!(NavCommand::ZoomIn.category(), CommandCategory::Display);
    }

    #[test]
    fn test_zoom_levels() {
        assert!((ZoomLevel::Overview.factor() - 0.25).abs() < 0.001);
        assert!((ZoomLevel::Standard.factor() - 1.0).abs() < 0.001);
        assert!((ZoomLevel::Detail.factor() - 2.0).abs() < 0.001);
        assert!((ZoomLevel::Custom(1.5).factor() - 1.5).abs() < 0.001);

        let zoomed_in = ZoomLevel::Standard.zoom_in();
        assert!(zoomed_in.factor() > 1.0);

        let zoomed_out = ZoomLevel::Standard.zoom_out();
        assert!(zoomed_out.factor() < 1.0);
    }
}
