//! Navigation Command Executor
//!
//! This module implements command execution against an EntityGraph.
//! Commands are parsed by the parser module and executed here.
//!
//! ## Design Principles
//!
//! 1. **Stateless execution** - Each command returns a result, graph state updated
//! 2. **Server is source of truth** - Load commands fetch from database
//! 3. **Clear result types** - NavResult enum describes all outcomes

use uuid::Uuid;

use super::commands::{NavCommand, ZoomLevel};
use crate::graph::{EntityGraph, EntityType, GraphFilters, GraphNode, GraphScope, RoleAssignment};

// =============================================================================
// NAVIGATION RESULT
// =============================================================================

/// Result of executing a navigation command
#[derive(Debug, Clone)]
pub enum NavResult {
    /// Successfully navigated from one node to another
    Navigated {
        from: Option<Uuid>,
        to: Uuid,
        node_name: String,
    },

    /// At the terminus (top of ownership chain) - cannot go up further
    AtTerminus,

    /// Node has no children - cannot go down
    NoChildren,

    /// No cursor set - cannot navigate relatively
    NoCursor,

    /// Entity not found
    NotFound { query: String },

    /// Filter was applied successfully
    FilterApplied { description: String },

    /// Scope was loaded successfully
    ScopeLoaded {
        scope: GraphScope,
        node_count: usize,
        edge_count: usize,
    },

    /// Query returned results
    QueryResult {
        query: String,
        results: Vec<QueryResultItem>,
    },

    /// Path from cursor to terminus
    PathResult { path: Vec<PathNode> },

    /// Context information about current node
    ContextResult {
        node: Box<GraphNode>,
        roles: Vec<RoleAssignment>,
        owner_count: usize,
        owned_count: usize,
        controller_count: usize,
    },

    /// Tree view from current node
    TreeResult { root: TreeNode },

    /// Zoom level changed
    ZoomChanged(ZoomLevel),

    /// CBU expanded/collapsed
    CbuToggled { cbu_id: Uuid, expanded: bool },

    /// Help text
    HelpText { topic: Option<String>, text: String },

    /// Command undone/redone
    UndoRedo { action: String },

    /// Error occurred
    Error { message: String },
}

/// Item in a query result
#[derive(Debug, Clone)]
pub struct QueryResultItem {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: EntityType,
    pub jurisdiction: Option<String>,
    pub relevance: Option<f32>,
    pub context: Option<String>,
}

/// Node in a path result
#[derive(Debug, Clone)]
pub struct PathNode {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: EntityType,
    pub depth: u32,
    pub ownership_pct: Option<rust_decimal::Decimal>,
}

/// Node in a tree result
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: EntityType,
    pub children: Vec<TreeNode>,
    pub ownership_pct: Option<rust_decimal::Decimal>,
}

// =============================================================================
// COMMAND EXECUTOR TRAIT
// =============================================================================

/// Trait for executing navigation commands
///
/// This is implemented by EntityGraph for in-memory operations.
/// Database-backed implementations would load data as needed.
pub trait NavExecutor {
    /// Execute a navigation command and return the result
    fn execute_nav(&mut self, cmd: NavCommand) -> NavResult;
}

impl NavExecutor for EntityGraph {
    fn execute_nav(&mut self, cmd: NavCommand) -> NavResult {
        match cmd {
            // =========================================================
            // SCOPE COMMANDS - These would typically reload from DB
            // =========================================================
            NavCommand::LoadCbu { cbu_name } => {
                // In a real implementation, this would load from database
                // For now, we just update the scope
                self.scope = GraphScope::SingleCbu {
                    cbu_id: Uuid::nil(), // Would be resolved from cbu_name
                    cbu_name: cbu_name.clone(),
                };
                NavResult::ScopeLoaded {
                    scope: self.scope.clone(),
                    node_count: self.nodes.len(),
                    edge_count: self.ownership_edges.len() + self.control_edges.len(),
                }
            }

            NavCommand::LoadBook { client_name } => {
                self.scope = GraphScope::Book {
                    apex_entity_id: Uuid::nil(), // Would be resolved
                    apex_name: client_name,
                };
                NavResult::ScopeLoaded {
                    scope: self.scope.clone(),
                    node_count: self.nodes.len(),
                    edge_count: self.ownership_edges.len() + self.control_edges.len(),
                }
            }

            NavCommand::LoadJurisdiction { code } => {
                self.scope = GraphScope::Jurisdiction { code: code.clone() };
                NavResult::ScopeLoaded {
                    scope: self.scope.clone(),
                    node_count: self.nodes.len(),
                    edge_count: self.ownership_edges.len() + self.control_edges.len(),
                }
            }

            NavCommand::LoadNeighborhood {
                entity_name: _,
                hops,
            } => {
                // Would resolve entity_name to ID and load neighborhood
                self.scope = GraphScope::EntityNeighborhood {
                    entity_id: Uuid::nil(),
                    hops,
                };
                NavResult::ScopeLoaded {
                    scope: self.scope.clone(),
                    node_count: self.nodes.len(),
                    edge_count: self.ownership_edges.len() + self.control_edges.len(),
                }
            }

            // =========================================================
            // FILTER COMMANDS
            // =========================================================
            NavCommand::FilterJurisdiction { codes } => {
                self.filters.jurisdictions = Some(codes.clone());
                self.recompute_visibility();
                NavResult::FilterApplied {
                    description: format!("Jurisdiction filter: {}", codes.join(", ")),
                }
            }

            NavCommand::FilterFundType { fund_types } => {
                self.filters.fund_types = Some(fund_types.clone());
                self.recompute_visibility();
                NavResult::FilterApplied {
                    description: format!("Fund type filter: {}", fund_types.join(", ")),
                }
            }

            NavCommand::FilterProng { prong } => {
                self.filters.prong = prong;
                self.recompute_visibility();
                NavResult::FilterApplied {
                    description: format!("Prong filter: {:?}", prong),
                }
            }

            NavCommand::FilterMinOwnership { percentage } => {
                self.filters.min_ownership_pct =
                    Some(rust_decimal::Decimal::from_f64_retain(percentage).unwrap_or_default());
                self.recompute_visibility();
                NavResult::FilterApplied {
                    description: format!("Minimum ownership: {}%", percentage),
                }
            }

            NavCommand::FilterPathOnly { enabled } => {
                self.filters.path_only = enabled;
                self.recompute_visibility();
                NavResult::FilterApplied {
                    description: format!("Path only: {}", enabled),
                }
            }

            NavCommand::ClearFilters => {
                self.filters = GraphFilters::default();
                self.recompute_visibility();
                NavResult::FilterApplied {
                    description: "Filters cleared".to_string(),
                }
            }

            NavCommand::AsOfDate { date } => {
                self.filters.as_of_date = date;
                self.recompute_visibility();
                NavResult::FilterApplied {
                    description: format!("As of date: {}", date),
                }
            }

            // =========================================================
            // NAVIGATION COMMANDS
            // =========================================================
            NavCommand::GoTo { entity_name } => {
                // Find entity by name
                if let Some((id, node)) = self
                    .nodes
                    .iter()
                    .find(|(_, n)| n.name.to_lowercase().contains(&entity_name.to_lowercase()))
                {
                    let from = self.cursor;
                    self.cursor = Some(*id);
                    if let Some(from_id) = from {
                        self.history.push(from_id);
                    }
                    NavResult::Navigated {
                        from,
                        to: *id,
                        node_name: node.name.clone(),
                    }
                } else {
                    NavResult::NotFound { query: entity_name }
                }
            }

            NavCommand::GoUp => {
                let Some(cursor_id) = self.cursor else {
                    return NavResult::NoCursor;
                };

                let Some(cursor_node) = self.nodes.get(&cursor_id) else {
                    return NavResult::Error {
                        message: "Cursor node not found".to_string(),
                    };
                };

                // Find first owner
                if let Some(&owner_id) = cursor_node.owners.first() {
                    if let Some(owner_node) = self.nodes.get(&owner_id) {
                        self.history.push(cursor_id);
                        self.cursor = Some(owner_id);
                        NavResult::Navigated {
                            from: Some(cursor_id),
                            to: owner_id,
                            node_name: owner_node.name.clone(),
                        }
                    } else {
                        NavResult::Error {
                            message: "Owner node not found".to_string(),
                        }
                    }
                } else {
                    NavResult::AtTerminus
                }
            }

            NavCommand::GoDown { index, name } => {
                let Some(cursor_id) = self.cursor else {
                    return NavResult::NoCursor;
                };

                let Some(cursor_node) = self.nodes.get(&cursor_id) else {
                    return NavResult::Error {
                        message: "Cursor node not found".to_string(),
                    };
                };

                if cursor_node.owned.is_empty() {
                    return NavResult::NoChildren;
                }

                // Find target child
                let target_id = if let Some(target_name) = name {
                    cursor_node.owned.iter().find(|&&id| {
                        self.nodes
                            .get(&id)
                            .map(|n| n.name.to_lowercase().contains(&target_name.to_lowercase()))
                            .unwrap_or(false)
                    })
                } else if let Some(idx) = index {
                    cursor_node.owned.get(idx)
                } else {
                    cursor_node.owned.first()
                };

                if let Some(&child_id) = target_id {
                    if let Some(child_node) = self.nodes.get(&child_id) {
                        self.history.push(cursor_id);
                        self.cursor = Some(child_id);
                        NavResult::Navigated {
                            from: Some(cursor_id),
                            to: child_id,
                            node_name: child_node.name.clone(),
                        }
                    } else {
                        NavResult::Error {
                            message: "Child node not found".to_string(),
                        }
                    }
                } else {
                    NavResult::NotFound {
                        query: "child".to_string(),
                    }
                }
            }

            NavCommand::GoSibling { direction } => {
                let Some(cursor_id) = self.cursor else {
                    return NavResult::NoCursor;
                };

                // Find parent and then sibling
                let cursor_node = match self.nodes.get(&cursor_id) {
                    Some(n) => n,
                    None => {
                        return NavResult::Error {
                            message: "Cursor node not found".to_string(),
                        }
                    }
                };

                if let Some(&parent_id) = cursor_node.owners.first() {
                    if let Some(parent_node) = self.nodes.get(&parent_id) {
                        if let Some(current_idx) =
                            parent_node.owned.iter().position(|&id| id == cursor_id)
                        {
                            let sibling_idx = match direction {
                                super::commands::Direction::Left
                                | super::commands::Direction::Prev => {
                                    if current_idx > 0 {
                                        current_idx - 1
                                    } else {
                                        parent_node.owned.len() - 1
                                    }
                                }
                                super::commands::Direction::Right
                                | super::commands::Direction::Next => {
                                    (current_idx + 1) % parent_node.owned.len()
                                }
                            };

                            if let Some(&sibling_id) = parent_node.owned.get(sibling_idx) {
                                if let Some(sibling_node) = self.nodes.get(&sibling_id) {
                                    self.history.push(cursor_id);
                                    self.cursor = Some(sibling_id);
                                    return NavResult::Navigated {
                                        from: Some(cursor_id),
                                        to: sibling_id,
                                        node_name: sibling_node.name.clone(),
                                    };
                                }
                            }
                        }
                    }
                }

                NavResult::NotFound {
                    query: "sibling".to_string(),
                }
            }

            NavCommand::GoToTerminus => {
                let Some(mut current_id) = self.cursor else {
                    return NavResult::NoCursor;
                };

                let from = self.cursor;

                // Walk up until we find a node with no owners
                while let Some(current_node) = self.nodes.get(&current_id) {
                    if current_node.owners.is_empty() {
                        break;
                    }

                    if let Some(&owner_id) = current_node.owners.first() {
                        current_id = owner_id;
                    } else {
                        break;
                    }
                }

                if let Some(terminus_node) = self.nodes.get(&current_id) {
                    if let Some(from_id) = from {
                        self.history.push(from_id);
                    }
                    self.cursor = Some(current_id);
                    NavResult::Navigated {
                        from,
                        to: current_id,
                        node_name: terminus_node.name.clone(),
                    }
                } else {
                    NavResult::Error {
                        message: "Terminus not found".to_string(),
                    }
                }
            }

            NavCommand::GoToClient => {
                // Navigate to commercial client (if scope is Book or SingleCbu)
                match &self.scope {
                    GraphScope::Book { apex_entity_id, .. } if *apex_entity_id != Uuid::nil() => {
                        if let Some(client_node) = self.nodes.get(apex_entity_id) {
                            let from = self.cursor;
                            if let Some(from_id) = from {
                                self.history.push(from_id);
                            }
                            self.cursor = Some(*apex_entity_id);
                            NavResult::Navigated {
                                from,
                                to: *apex_entity_id,
                                node_name: client_node.name.clone(),
                            }
                        } else {
                            NavResult::NotFound {
                                query: "commercial client".to_string(),
                            }
                        }
                    }
                    _ => NavResult::Error {
                        message: "No commercial client in current scope".to_string(),
                    },
                }
            }

            NavCommand::GoBack => {
                if let Some(prev_id) = self.history.go_back(self.cursor) {
                    if let Some(prev_node) = self.nodes.get(&prev_id) {
                        let from = self.cursor;
                        self.cursor = Some(prev_id);
                        NavResult::Navigated {
                            from,
                            to: prev_id,
                            node_name: prev_node.name.clone(),
                        }
                    } else {
                        NavResult::Error {
                            message: "Previous node not found".to_string(),
                        }
                    }
                } else {
                    NavResult::Error {
                        message: "No navigation history".to_string(),
                    }
                }
            }

            NavCommand::GoForward => {
                if let Some(next_id) = self.history.go_forward(self.cursor) {
                    if let Some(next_node) = self.nodes.get(&next_id) {
                        let from = self.cursor;
                        self.cursor = Some(next_id);
                        NavResult::Navigated {
                            from,
                            to: next_id,
                            node_name: next_node.name.clone(),
                        }
                    } else {
                        NavResult::Error {
                            message: "Forward node not found".to_string(),
                        }
                    }
                } else {
                    NavResult::Error {
                        message: "No forward history".to_string(),
                    }
                }
            }

            // =========================================================
            // QUERY COMMANDS
            // =========================================================
            NavCommand::Find { name_pattern } => {
                let pattern = name_pattern.to_lowercase();
                let results: Vec<QueryResultItem> = self
                    .nodes
                    .values()
                    .filter(|n| n.name.to_lowercase().contains(&pattern))
                    .map(|n| QueryResultItem {
                        entity_id: n.entity_id,
                        name: n.name.clone(),
                        entity_type: n.entity_type,
                        jurisdiction: n.jurisdiction.clone(),
                        relevance: None,
                        context: None,
                    })
                    .collect();

                NavResult::QueryResult {
                    query: name_pattern,
                    results,
                }
            }

            NavCommand::WhereIs { person_name, role } => {
                let pattern = person_name.to_lowercase();
                let results: Vec<QueryResultItem> = self
                    .role_assignments
                    .iter()
                    .filter(|ra| {
                        if let Some(node) = self.nodes.get(&ra.entity_id) {
                            let name_match = node.name.to_lowercase().contains(&pattern);
                            let role_match = role
                                .as_ref()
                                .map(|r| ra.role.to_lowercase().contains(&r.to_lowercase()))
                                .unwrap_or(true);
                            name_match && role_match
                        } else {
                            false
                        }
                    })
                    .filter_map(|ra| {
                        self.nodes.get(&ra.entity_id).map(|n| QueryResultItem {
                            entity_id: n.entity_id,
                            name: n.name.clone(),
                            entity_type: n.entity_type,
                            jurisdiction: n.jurisdiction.clone(),
                            relevance: None,
                            context: Some(format!(
                                "{} at {}",
                                ra.role,
                                self.cbus
                                    .get(&ra.cbu_id)
                                    .map(|c| c.name.as_str())
                                    .unwrap_or("?")
                            )),
                        })
                    })
                    .collect();

                NavResult::QueryResult {
                    query: format!("{} {}", person_name, role.unwrap_or_default()),
                    results,
                }
            }

            NavCommand::FindByRole { role } => {
                let pattern = role.to_lowercase();
                let results: Vec<QueryResultItem> = self
                    .role_assignments
                    .iter()
                    .filter(|ra| ra.role.to_lowercase().contains(&pattern))
                    .filter_map(|ra| {
                        self.nodes.get(&ra.entity_id).map(|n| QueryResultItem {
                            entity_id: n.entity_id,
                            name: n.name.clone(),
                            entity_type: n.entity_type,
                            jurisdiction: n.jurisdiction.clone(),
                            relevance: None,
                            context: Some(ra.role.clone()),
                        })
                    })
                    .collect();

                NavResult::QueryResult {
                    query: format!("role:{}", role),
                    results,
                }
            }

            NavCommand::ListChildren => {
                let Some(cursor_id) = self.cursor else {
                    return NavResult::NoCursor;
                };

                let results: Vec<QueryResultItem> = self
                    .nodes
                    .get(&cursor_id)
                    .map(|n| &n.owned)
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|&id| {
                        self.nodes.get(&id).map(|n| QueryResultItem {
                            entity_id: n.entity_id,
                            name: n.name.clone(),
                            entity_type: n.entity_type,
                            jurisdiction: n.jurisdiction.clone(),
                            relevance: None,
                            context: None,
                        })
                    })
                    .collect();

                NavResult::QueryResult {
                    query: "children".to_string(),
                    results,
                }
            }

            NavCommand::ListOwners => {
                let Some(cursor_id) = self.cursor else {
                    return NavResult::NoCursor;
                };

                let results: Vec<QueryResultItem> = self
                    .nodes
                    .get(&cursor_id)
                    .map(|n| &n.owners)
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|&id| {
                        self.nodes.get(&id).map(|n| QueryResultItem {
                            entity_id: n.entity_id,
                            name: n.name.clone(),
                            entity_type: n.entity_type,
                            jurisdiction: n.jurisdiction.clone(),
                            relevance: None,
                            context: None,
                        })
                    })
                    .collect();

                NavResult::QueryResult {
                    query: "owners".to_string(),
                    results,
                }
            }

            NavCommand::ListControllers => {
                let Some(cursor_id) = self.cursor else {
                    return NavResult::NoCursor;
                };

                let results: Vec<QueryResultItem> = self
                    .nodes
                    .get(&cursor_id)
                    .map(|n| &n.controlled_by)
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|&id| {
                        self.nodes.get(&id).map(|n| QueryResultItem {
                            entity_id: n.entity_id,
                            name: n.name.clone(),
                            entity_type: n.entity_type,
                            jurisdiction: n.jurisdiction.clone(),
                            relevance: None,
                            context: None,
                        })
                    })
                    .collect();

                NavResult::QueryResult {
                    query: "controllers".to_string(),
                    results,
                }
            }

            NavCommand::ListCbus => {
                let results: Vec<QueryResultItem> = self
                    .cbus
                    .values()
                    .map(|c| QueryResultItem {
                        entity_id: c.cbu_id,
                        name: c.name.clone(),
                        entity_type: EntityType::Unknown, // CBU is not an entity type
                        jurisdiction: c.jurisdiction.clone(),
                        relevance: None,
                        context: Some(format!("{:?}", c.status)),
                    })
                    .collect();

                NavResult::QueryResult {
                    query: "cbus".to_string(),
                    results,
                }
            }

            // =========================================================
            // DISPLAY COMMANDS
            // =========================================================
            NavCommand::ShowPath => {
                let Some(cursor_id) = self.cursor else {
                    return NavResult::NoCursor;
                };

                let mut path = Vec::new();
                let mut current_id = cursor_id;
                let mut depth = 0u32;

                while let Some(node) = self.nodes.get(&current_id) {
                    // Find ownership percentage from edge
                    let ownership_pct = if depth > 0 {
                        self.ownership_edges
                            .iter()
                            .find(|e| e.to_entity_id == current_id)
                            .map(|e| e.percentage)
                    } else {
                        None
                    };

                    path.push(PathNode {
                        entity_id: current_id,
                        name: node.name.clone(),
                        entity_type: node.entity_type,
                        depth,
                        ownership_pct,
                    });

                    if let Some(&owner_id) = node.owners.first() {
                        current_id = owner_id;
                        depth += 1;
                    } else {
                        break;
                    }

                    if depth > 50 {
                        break; // Prevent infinite loops
                    }
                }

                path.reverse(); // Top to bottom
                NavResult::PathResult { path }
            }

            NavCommand::ShowContext => {
                let Some(cursor_id) = self.cursor else {
                    return NavResult::NoCursor;
                };

                if let Some(node) = self.nodes.get(&cursor_id).cloned() {
                    let roles: Vec<RoleAssignment> = self
                        .role_assignments
                        .iter()
                        .filter(|ra| ra.entity_id == cursor_id)
                        .cloned()
                        .collect();

                    NavResult::ContextResult {
                        node: Box::new(node.clone()),
                        roles,
                        owner_count: node.owners.len(),
                        owned_count: node.owned.len(),
                        controller_count: node.controlled_by.len(),
                    }
                } else {
                    NavResult::Error {
                        message: "Cursor node not found".to_string(),
                    }
                }
            }

            NavCommand::ShowTree { depth } => {
                let Some(cursor_id) = self.cursor else {
                    return NavResult::NoCursor;
                };

                fn build_tree(
                    graph: &EntityGraph,
                    node_id: Uuid,
                    remaining_depth: u32,
                ) -> Option<TreeNode> {
                    let node = graph.nodes.get(&node_id)?;

                    let children = if remaining_depth > 0 {
                        node.owned
                            .iter()
                            .filter_map(|&child_id| {
                                build_tree(graph, child_id, remaining_depth - 1)
                            })
                            .collect()
                    } else {
                        vec![]
                    };

                    let ownership_pct = graph
                        .ownership_edges
                        .iter()
                        .find(|e| e.to_entity_id == node_id)
                        .map(|e| e.percentage);

                    Some(TreeNode {
                        entity_id: node_id,
                        name: node.name.clone(),
                        entity_type: node.entity_type,
                        children,
                        ownership_pct,
                    })
                }

                if let Some(root) = build_tree(self, cursor_id, depth) {
                    NavResult::TreeResult { root }
                } else {
                    NavResult::Error {
                        message: "Could not build tree".to_string(),
                    }
                }
            }

            NavCommand::ExpandCbu { cbu_name } => {
                let target_cbu = if let Some(name) = cbu_name {
                    self.cbus.values_mut().find(|c| c.name.contains(&name))
                } else if let Some(cursor_id) = self.cursor {
                    // Find CBU containing cursor
                    self.cbus
                        .values_mut()
                        .find(|c| c.member_entities.contains(&cursor_id))
                } else {
                    None
                };

                if let Some(cbu) = target_cbu {
                    cbu.expanded = true;
                    NavResult::CbuToggled {
                        cbu_id: cbu.cbu_id,
                        expanded: true,
                    }
                } else {
                    NavResult::NotFound {
                        query: "CBU".to_string(),
                    }
                }
            }

            NavCommand::CollapseCbu { cbu_name } => {
                let target_cbu = if let Some(name) = cbu_name {
                    self.cbus.values_mut().find(|c| c.name.contains(&name))
                } else if let Some(cursor_id) = self.cursor {
                    self.cbus
                        .values_mut()
                        .find(|c| c.member_entities.contains(&cursor_id))
                } else {
                    None
                };

                if let Some(cbu) = target_cbu {
                    cbu.expanded = false;
                    NavResult::CbuToggled {
                        cbu_id: cbu.cbu_id,
                        expanded: false,
                    }
                } else {
                    NavResult::NotFound {
                        query: "CBU".to_string(),
                    }
                }
            }

            NavCommand::Zoom { level } => NavResult::ZoomChanged(level),

            NavCommand::ZoomIn => NavResult::ZoomChanged(ZoomLevel::Standard.zoom_in()),

            NavCommand::ZoomOut => NavResult::ZoomChanged(ZoomLevel::Standard.zoom_out()),

            NavCommand::FitToView => NavResult::ZoomChanged(ZoomLevel::Overview),

            // =========================================================
            // META COMMANDS
            // =========================================================
            NavCommand::Help { topic } => {
                let text = generate_help_text(topic.as_deref());
                NavResult::HelpText { topic, text }
            }

            NavCommand::Undo => {
                // Undo would typically be handled at a higher level
                NavResult::UndoRedo {
                    action: "undo".to_string(),
                }
            }

            NavCommand::Redo => NavResult::UndoRedo {
                action: "redo".to_string(),
            },
        }
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

impl EntityGraph {
    fn recompute_visibility(&mut self) {
        // Use the filter logic from filters module
        use crate::graph::filters::GraphFilterOps;
        GraphFilterOps::recompute_visibility(self);
    }
}

fn generate_help_text(topic: Option<&str>) -> String {
    match topic {
        Some("navigation") | Some("nav") => r#"Navigation Commands:
  go up / up / parent    - Navigate to owner
  go down / down / child - Navigate to owned entity
  left / right           - Navigate to sibling
  terminus / top         - Navigate to ownership apex
  back / <               - Go back in history
  forward / >            - Go forward in history"#
            .to_string(),
        Some("filter") | Some("filters") => r#"Filter Commands:
  show ownership         - Show only ownership relationships
  show control           - Show only control relationships
  show both              - Show both prongs
  filter jurisdiction X  - Filter to jurisdiction
  min ownership 25%      - Minimum ownership threshold
  clear filters          - Remove all filters
  as of 2024-01-01       - Temporal filter"#
            .to_string(),
        Some("query") | Some("search") => r#"Query Commands:
  find "Pattern"         - Search entities by name
  where is "Person"      - Find person's roles
  find by role X         - Find entities with role
  list children          - List owned entities
  list owners            - List owning entities
  list cbus              - List all CBUs"#
            .to_string(),
        _ => r#"Navigation Help:
  help navigation  - Movement commands
  help filter      - Filter commands
  help query       - Search commands

Quick Reference:
  up/down          - Navigate ownership chain
  back/forward     - History navigation
  find "X"         - Search by name
  show ownership   - Filter to ownership prong"#
            .to_string(),
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_generation() {
        let general = generate_help_text(None);
        assert!(general.contains("Navigation Help"));

        let nav = generate_help_text(Some("navigation"));
        assert!(nav.contains("go up"));

        let filter = generate_help_text(Some("filter"));
        assert!(filter.contains("show ownership"));
    }
}
