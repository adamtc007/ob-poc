//! Agent Context for Graph Navigation
//!
//! Provides structured context for LLM agents to understand:
//! - Current position and state
//! - What's visible and what's off-screen
//! - What commands make sense now
//!
//! # Usage
//!
//! ```rust,ignore
//! let session = UnifiedSessionContext::new();
//! // ... load graph and set cursor ...
//! let context = AgentGraphContext::from_session(&session);
//! let prompt_text = context.to_prompt_text();
//! ```

use serde::Serialize;

use crate::graph::{GraphScope, ProngFilter};
use crate::session::UnifiedSessionContext;

/// Context injected into agent prompts for graph navigation
#[derive(Debug, Clone, Serialize)]
pub struct AgentGraphContext {
    /// Current scope summary
    pub scope: ScopeSummaryForAgent,

    /// Current cursor position (if set)
    pub cursor: Option<CursorContext>,

    /// What's around the cursor
    pub neighborhood: Option<NeighborhoodContext>,

    /// Active filters
    pub filters: FilterContext,

    /// Viewport state
    pub viewport: ViewportForAgent,

    /// Commands that make sense given current state
    pub suggested_commands: Vec<SuggestedCommand>,

    /// DSL bindings available (from ExecutionContext)
    pub bindings: Vec<BindingSummary>,
}

/// Scope summary formatted for agent consumption
#[derive(Debug, Clone, Serialize)]
pub struct ScopeSummaryForAgent {
    /// Scope type (SingleCbu, Book, Jurisdiction, etc.)
    pub scope_type: String,
    /// Scope name (fund name, client name, jurisdiction code)
    pub scope_name: String,
    /// Total entities in scope
    pub total_entities: usize,
    /// Total CBUs in scope
    pub total_cbus: usize,
    /// Top jurisdictions represented
    pub jurisdictions: Vec<String>,
}

/// Current cursor position context
#[derive(Debug, Clone, Serialize)]
pub struct CursorContext {
    /// Entity UUID as string
    pub entity_id: String,
    /// Display name
    pub name: String,
    /// Entity type (Fund, LimitedCompany, ProperPerson, etc.)
    pub entity_type: String,
    /// Jurisdiction code
    pub jurisdiction: Option<String>,
    /// Depth from nearest terminus
    pub depth_from_terminus: u32,
    /// Whether this entity is a terminus (UBO, public company)
    pub is_terminus: bool,
    /// Whether this is a natural person
    pub is_natural_person: bool,
}

/// Neighborhood around current cursor
#[derive(Debug, Clone, Serialize)]
pub struct NeighborhoodContext {
    /// Parent owners (can go up to these)
    pub owners: Vec<NeighborSummary>,
    /// Owned children (can go down to these)
    pub children: Vec<NeighborSummary>,
    /// Entities that control this one
    pub controllers: Vec<NeighborSummary>,
    /// Entities controlled by this one
    pub controlled: Vec<NeighborSummary>,
    /// Count if children were truncated
    pub children_truncated: Option<usize>,
}

/// Summary of a neighboring entity
#[derive(Debug, Clone, Serialize)]
pub struct NeighborSummary {
    /// Display name
    pub name: String,
    /// Entity type
    pub entity_type: String,
    /// Relationship hint (e.g., "100% owner", "Chairman")
    pub hint: Option<String>,
}

/// Active filter context
#[derive(Debug, Clone, Serialize)]
pub struct FilterContext {
    /// Active prong filter (both, ownership, control)
    pub prong: String,
    /// Jurisdiction filter if any
    pub jurisdictions: Option<Vec<String>>,
    /// As-of date for temporal filtering
    pub as_of_date: String,
    /// Minimum ownership percentage filter
    pub min_ownership_pct: Option<f64>,
}

/// Viewport state for agent
#[derive(Debug, Clone, Serialize)]
pub struct ViewportForAgent {
    /// Zoom level name (Overview, Standard, Detail)
    pub zoom_level: String,
    /// Number of visible entities
    pub visible_count: usize,
    /// Entities above viewport
    pub off_screen_above: usize,
    /// Entities below viewport
    pub off_screen_below: usize,
    /// Entities to the left
    pub off_screen_left: usize,
    /// Entities to the right
    pub off_screen_right: usize,
    /// Natural language hint about off-screen content
    pub off_screen_hint: Option<String>,
}

/// A suggested command with relevance score
#[derive(Debug, Clone, Serialize)]
pub struct SuggestedCommand {
    /// Command syntax
    pub command: String,
    /// Description of what it does
    pub description: String,
    /// Relevance score (0.0 - 1.0)
    pub relevance: f32,
}

/// Summary of a DSL binding
#[derive(Debug, Clone, Serialize)]
pub struct BindingSummary {
    /// Binding name (e.g., "@fund")
    pub name: String,
    /// Binding type (cbu, entity, etc.)
    pub binding_type: String,
    /// Display name if available
    pub display_name: Option<String>,
}

impl AgentGraphContext {
    /// Build context from unified session
    pub fn from_session(session: &UnifiedSessionContext) -> Self {
        let scope = Self::build_scope_summary(session);
        let cursor = Self::build_cursor_context(session);
        let neighborhood = Self::build_neighborhood(session);
        let filters = Self::build_filter_context(session);
        let viewport = Self::build_viewport_context(session);
        let suggested = Self::compute_suggestions(session);
        let bindings = Self::build_bindings(session);

        Self {
            scope,
            cursor,
            neighborhood,
            filters,
            viewport,
            suggested_commands: suggested,
            bindings,
        }
    }

    fn build_scope_summary(session: &UnifiedSessionContext) -> ScopeSummaryForAgent {
        let (scope_type, scope_name) = match &session.scope.definition {
            GraphScope::SingleCbu { cbu_name, .. } => ("SingleCbu".into(), cbu_name.clone()),
            GraphScope::Book { apex_name, .. } => ("Book".into(), apex_name.clone()),
            GraphScope::Jurisdiction { code } => ("Jurisdiction".into(), code.clone()),
            GraphScope::EntityNeighborhood { entity_id, hops } => (
                "EntityNeighborhood".into(),
                format!("{} ({} hops)", entity_id, hops),
            ),
            GraphScope::Empty => ("Empty".into(), "None".into()),
            GraphScope::Custom { description } => ("Custom".into(), description.clone()),
        };

        let jurisdictions: Vec<String> = session
            .scope
            .stats
            .by_jurisdiction
            .keys()
            .take(5)
            .cloned()
            .collect();

        ScopeSummaryForAgent {
            scope_type,
            scope_name,
            total_entities: session.scope.stats.total_entities,
            total_cbus: session.scope.stats.total_cbus,
            jurisdictions,
        }
    }

    fn build_cursor_context(session: &UnifiedSessionContext) -> Option<CursorContext> {
        let graph = session.graph.as_ref()?;
        let cursor_id = graph.cursor?;
        let node = graph.nodes.get(&cursor_id)?;

        Some(CursorContext {
            entity_id: cursor_id.to_string(),
            name: node.name.clone(),
            entity_type: format!("{:?}", node.entity_type),
            jurisdiction: node.jurisdiction.clone(),
            depth_from_terminus: node.depth_from_terminus.unwrap_or(0),
            is_terminus: graph.termini.contains(&cursor_id),
            is_natural_person: node.is_natural_person,
        })
    }

    fn build_neighborhood(session: &UnifiedSessionContext) -> Option<NeighborhoodContext> {
        let graph = session.graph.as_ref()?;
        let cursor_id = graph.cursor?;
        let node = graph.nodes.get(&cursor_id)?;

        let max_show = 5;

        let owners: Vec<NeighborSummary> = node
            .owners
            .iter()
            .filter_map(|id| graph.nodes.get(id))
            .take(max_show)
            .map(|n| NeighborSummary {
                name: n.name.clone(),
                entity_type: format!("{:?}", n.entity_type),
                hint: None, // TODO: Add ownership percentage from edges
            })
            .collect();

        let children: Vec<NeighborSummary> = node
            .owned
            .iter()
            .filter_map(|id| graph.nodes.get(id))
            .take(max_show)
            .map(|n| NeighborSummary {
                name: n.name.clone(),
                entity_type: format!("{:?}", n.entity_type),
                hint: None,
            })
            .collect();

        let children_truncated = if node.owned.len() > max_show {
            Some(node.owned.len() - max_show)
        } else {
            None
        };

        // TODO: Add controllers/controlled from control edges
        Some(NeighborhoodContext {
            owners,
            children,
            controllers: vec![],
            controlled: vec![],
            children_truncated,
        })
    }

    fn build_filter_context(session: &UnifiedSessionContext) -> FilterContext {
        let filters = session
            .graph
            .as_ref()
            .map(|g| g.filters.clone())
            .unwrap_or_default();

        FilterContext {
            prong: match filters.prong {
                ProngFilter::Both => "both",
                ProngFilter::OwnershipOnly => "ownership",
                ProngFilter::ControlOnly => "control",
            }
            .to_string(),
            jurisdictions: filters.jurisdictions,
            as_of_date: filters.as_of_date.to_string(),
            min_ownership_pct: filters
                .min_ownership_pct
                .and_then(|d| d.to_string().parse().ok()),
        }
    }

    fn build_viewport_context(session: &UnifiedSessionContext) -> ViewportForAgent {
        let vp = &session.viewport;

        let off_screen_hint = if vp.off_screen.below > 0 {
            Some(format!("{} entities below", vp.off_screen.below))
        } else if vp.off_screen.above > 0 {
            Some(format!("{} entities above", vp.off_screen.above))
        } else {
            None
        };

        ViewportForAgent {
            zoom_level: format!("{:?}", vp.zoom_name),
            visible_count: vp.visible_entities.len(),
            off_screen_above: vp.off_screen.above,
            off_screen_below: vp.off_screen.below,
            off_screen_left: vp.off_screen.left,
            off_screen_right: vp.off_screen.right,
            off_screen_hint,
        }
    }

    fn compute_suggestions(session: &UnifiedSessionContext) -> Vec<SuggestedCommand> {
        let mut suggestions = Vec::new();

        // Check if we have a graph and cursor
        if let Some(graph) = &session.graph {
            if let Some(cursor_id) = graph.cursor {
                if let Some(node) = graph.nodes.get(&cursor_id) {
                    // Can go up?
                    if !node.owners.is_empty() {
                        let parent_name = node
                            .owners
                            .first()
                            .and_then(|id| graph.nodes.get(id))
                            .map(|n| n.name.as_str())
                            .unwrap_or("parent");
                        suggestions.push(SuggestedCommand {
                            command: "go up".into(),
                            description: format!("Navigate to {} (owner)", parent_name),
                            relevance: 0.9,
                        });
                    }

                    // Can go down?
                    if !node.owned.is_empty() {
                        suggestions.push(SuggestedCommand {
                            command: format!("go down ({})", node.owned.len()),
                            description: format!(
                                "Navigate to one of {} owned entities",
                                node.owned.len()
                            ),
                            relevance: 0.8,
                        });
                    }

                    // At terminus?
                    if graph.termini.contains(&cursor_id) {
                        suggestions.push(SuggestedCommand {
                            command: "show tree 3".into(),
                            description: "Show ownership tree from this terminus".into(),
                            relevance: 0.7,
                        });
                    }

                    // Has history?
                    if graph.history.can_go_back() {
                        suggestions.push(SuggestedCommand {
                            command: "back".into(),
                            description: "Go to previous position".into(),
                            relevance: 0.5,
                        });
                    }
                }
            } else {
                // No cursor - suggest setting one
                suggestions.push(SuggestedCommand {
                    command: "go to [entity name]".into(),
                    description: "Set cursor on an entity to navigate".into(),
                    relevance: 1.0,
                });
            }

            // Viewport suggestions
            let vp = &session.viewport;
            if vp.off_screen.below > 5 {
                suggestions.push(SuggestedCommand {
                    command: "pan down".into(),
                    description: format!("See {} more entities below", vp.off_screen.below),
                    relevance: 0.6,
                });
            }
            if vp.off_screen.above > 5 {
                suggestions.push(SuggestedCommand {
                    command: "pan up".into(),
                    description: format!("See {} more entities above", vp.off_screen.above),
                    relevance: 0.6,
                });
            }
        } else {
            // No graph loaded
            suggestions.push(SuggestedCommand {
                command: "load cbu [name]".into(),
                description: "Load a CBU to start navigating".into(),
                relevance: 1.0,
            });
            suggestions.push(SuggestedCommand {
                command: "load book [client name]".into(),
                description: "Load all CBUs under a commercial client".into(),
                relevance: 0.9,
            });
        }

        suggestions
    }

    fn build_bindings(session: &UnifiedSessionContext) -> Vec<BindingSummary> {
        session
            .execution
            .symbols
            .iter()
            .map(|(name, _uuid)| BindingSummary {
                name: format!("@{}", name),
                binding_type: session
                    .execution
                    .symbol_types
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| "unknown".into()),
                display_name: None, // TODO: Look up display name from graph
            })
            .collect()
    }

    /// Format as JSON for injection into agent prompt
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    /// Format as concise text for agent prompt
    pub fn to_prompt_text(&self) -> String {
        let mut parts = Vec::new();

        // Scope
        parts.push(format!(
            "[SCOPE: {} \"{}\" - {} entities, {} CBUs]",
            self.scope.scope_type,
            self.scope.scope_name,
            self.scope.total_entities,
            self.scope.total_cbus
        ));

        // Cursor
        if let Some(cursor) = &self.cursor {
            parts.push(format!(
                "[CURSOR: {} ({}) depth={} {}]",
                cursor.name,
                cursor.entity_type,
                cursor.depth_from_terminus,
                if cursor.is_terminus { "TERMINUS" } else { "" }
            ));
        } else {
            parts.push("[CURSOR: None - use 'go to X' to set]".into());
        }

        // Neighborhood
        if let Some(hood) = &self.neighborhood {
            if !hood.owners.is_empty() {
                let owners: Vec<&str> = hood.owners.iter().map(|n| n.name.as_str()).collect();
                parts.push(format!("[OWNERS: {}]", owners.join(", ")));
            }
            if !hood.children.is_empty() {
                let count = hood.children.len() + hood.children_truncated.unwrap_or(0);
                parts.push(format!("[CHILDREN: {} entities]", count));
            }
        }

        // Viewport
        parts.push(format!(
            "[VIEW: {} - {} visible, off-screen: ^{} v{} <{} >{}]",
            self.viewport.zoom_level,
            self.viewport.visible_count,
            self.viewport.off_screen_above,
            self.viewport.off_screen_below,
            self.viewport.off_screen_left,
            self.viewport.off_screen_right
        ));

        // Suggestions (top 3)
        if !self.suggested_commands.is_empty() {
            let cmds: Vec<String> = self
                .suggested_commands
                .iter()
                .take(3)
                .map(|s| format!("'{}': {}", s.command, s.description))
                .collect();
            parts.push(format!("[SUGGESTED: {}]", cmds.join(" | ")));
        }

        // Bindings
        if !self.bindings.is_empty() {
            let binds: Vec<&str> = self.bindings.iter().map(|b| b.name.as_str()).collect();
            parts.push(format!("[BINDINGS: {}]", binds.join(", ")));
        }

        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_session_context() {
        let session = UnifiedSessionContext::new();
        let context = AgentGraphContext::from_session(&session);

        assert_eq!(context.scope.scope_type, "Empty");
        assert!(context.cursor.is_none());
        assert!(context.neighborhood.is_none());
        assert!(!context.suggested_commands.is_empty());

        // Should suggest loading a scope
        assert!(context
            .suggested_commands
            .iter()
            .any(|s| s.command.contains("load")));
    }

    #[test]
    fn test_to_prompt_text() {
        let session = UnifiedSessionContext::new();
        let context = AgentGraphContext::from_session(&session);
        let text = context.to_prompt_text();

        assert!(text.contains("[SCOPE:"));
        assert!(text.contains("[CURSOR: None"));
        assert!(text.contains("[VIEW:"));
        assert!(text.contains("[SUGGESTED:"));
    }

    #[test]
    fn test_to_json() {
        let session = UnifiedSessionContext::new();
        let context = AgentGraphContext::from_session(&session);
        let json = context.to_json();

        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("scope").is_some());
        assert!(parsed.get("viewport").is_some());
        assert!(parsed.get("suggested_commands").is_some());
    }
}
