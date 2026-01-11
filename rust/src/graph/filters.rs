//! Graph Filter Logic
//!
//! This module provides filter application logic for the unified EntityGraph.
//! Filters control visibility of nodes and edges based on:
//! - Temporal: effective as of a specific date
//! - Jurisdiction: restrict to specific jurisdictions
//! - Prong: ownership-only, control-only, or both
//! - Percentage: minimum ownership threshold
//! - Path-only: show only nodes on path to cursor
//!
//! ## Design Principles (from EGUI-RULES.md)
//!
//! 1. **Server is source of truth** - Filters are applied server-side
//! 2. **Deterministic** - Same filters + same data = same visibility
//! 3. **Efficient** - Visibility can be recomputed incrementally

use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use super::types::{
    ControlEdge, EntityGraph, EntityType, GraphFilters, GraphNode, OwnershipEdge, ProngFilter,
};

// =============================================================================
// FILTER APPLICATION TRAIT
// =============================================================================

/// Extension trait for filter operations on EntityGraph
pub trait GraphFilterOps {
    /// Check if an ownership edge passes current filters
    fn edge_visible_ownership(&self, edge: &OwnershipEdge) -> bool;

    /// Check if a control edge passes current filters
    fn edge_visible_control(&self, edge: &ControlEdge) -> bool;

    /// Check if a node passes current filters
    fn node_visible(&self, node: &GraphNode) -> bool;

    /// Recompute visibility for all nodes/edges after filter change
    fn recompute_visibility(&mut self);

    /// Get visible children of a node (entities owned by this node)
    fn visible_children(&self, entity_id: Uuid) -> Vec<Uuid>;

    /// Get visible parents of a node (entities that own this node)
    fn visible_parents(&self, entity_id: Uuid) -> Vec<Uuid>;

    /// Get visible controllers of a node
    fn visible_controllers(&self, entity_id: Uuid) -> Vec<Uuid>;

    /// Get entities this node controls
    fn visible_controlled(&self, entity_id: Uuid) -> Vec<Uuid>;
}

impl GraphFilterOps for EntityGraph {
    fn edge_visible_ownership(&self, edge: &OwnershipEdge) -> bool {
        // Check prong filter
        if matches!(self.filters.prong, ProngFilter::ControlOnly) {
            return false;
        }

        // Check temporal filter - edge must be effective as of the filter date
        if !edge_effective_at(
            &edge.effective_from,
            &edge.effective_to,
            self.filters.as_of_date,
        ) {
            return false;
        }

        // Check minimum ownership percentage
        if let Some(min_pct) = &self.filters.min_ownership_pct {
            if &edge.percentage < min_pct {
                return false;
            }
        }

        // Check if source and target nodes pass filters
        let source_visible = self
            .nodes
            .get(&edge.from_entity_id)
            .map(|n| self.node_visible(n))
            .unwrap_or(false);

        let target_visible = self
            .nodes
            .get(&edge.to_entity_id)
            .map(|n| self.node_visible(n))
            .unwrap_or(false);

        source_visible && target_visible
    }

    fn edge_visible_control(&self, edge: &ControlEdge) -> bool {
        // Check prong filter
        if matches!(self.filters.prong, ProngFilter::OwnershipOnly) {
            return false;
        }

        // Check temporal filter
        if !edge_effective_at(
            &edge.effective_from,
            &edge.effective_to,
            self.filters.as_of_date,
        ) {
            return false;
        }

        // Check if source and target nodes pass filters
        let source_visible = self
            .nodes
            .get(&edge.controller_id)
            .map(|n| self.node_visible(n))
            .unwrap_or(false);

        let target_visible = self
            .nodes
            .get(&edge.controlled_id)
            .map(|n| self.node_visible(n))
            .unwrap_or(false);

        source_visible && target_visible
    }

    fn node_visible(&self, node: &GraphNode) -> bool {
        // Check jurisdiction filter
        if let Some(ref allowed_jurisdictions) = self.filters.jurisdictions {
            if let Some(ref node_jurisdiction) = node.jurisdiction {
                if !allowed_jurisdictions.contains(node_jurisdiction) {
                    return false;
                }
            }
            // Nodes without jurisdiction are hidden if jurisdiction filter is set
            // (except natural persons which often lack jurisdiction)
            else if !node.is_natural_person {
                return false;
            }
        }

        // Check entity type filter
        if let Some(ref allowed_types) = self.filters.entity_types {
            if !allowed_types.contains(&node.entity_type) {
                return false;
            }
        }

        // Check fund type filter (only applies to fund entities)
        // Note: EntityType variants like Fund, Sicav, Icav, etc. represent fund types
        if let Some(ref allowed_fund_types) = self.filters.fund_types {
            let is_fund = matches!(
                node.entity_type,
                EntityType::Fund
                    | EntityType::Sicav
                    | EntityType::Icav
                    | EntityType::Oeic
                    | EntityType::Vcc
                    | EntityType::UnitTrust
                    | EntityType::Fcp
            );
            if is_fund {
                let fund_type_str = format!("{:?}", node.entity_type);
                if !allowed_fund_types
                    .iter()
                    .any(|ft| ft.eq_ignore_ascii_case(&fund_type_str))
                {
                    return false;
                }
            }
        }

        // If path_only is set, node must be on path to cursor
        if self.filters.path_only {
            if let Some(cursor_id) = self.cursor {
                if node.entity_id != cursor_id && !self.is_on_path_to_cursor(node.entity_id) {
                    return false;
                }
            }
        }

        // Check Same ManCo filter - entity must be managed by the same ManCo
        if let Some(manco_id) = self.filters.same_manco_id {
            // If node has a manco_id, it must match. If node has no manco_id, show it anyway
            // (it could be the ManCo itself, or an entity not in the fund structure)
            if let Some(node_manco) = node.manco_id {
                if node_manco != manco_id {
                    return false;
                }
            }
        }

        // Check Same SICAV filter - entity must belong to the same SICAV/umbrella
        if let Some(sicav_id) = self.filters.same_sicav_id {
            // If node has a sicav_id, it must match. If node has no sicav_id, show it anyway
            // (it could be the SICAV itself, or an entity not in the fund structure)
            if let Some(node_sicav) = node.sicav_id {
                if node_sicav != sicav_id {
                    return false;
                }
            }
        }

        true
    }

    fn recompute_visibility(&mut self) {
        // First pass: compute node visibility
        let node_visibility: std::collections::HashMap<Uuid, bool> = self
            .nodes
            .iter()
            .map(|(id, node)| (*id, self.node_visible(node)))
            .collect();

        // Update node visible flags
        for (id, visible) in &node_visibility {
            if let Some(node) = self.nodes.get_mut(id) {
                node.visible = *visible;
            }
        }

        // Note: Edge visibility is computed on-demand via edge_visible_* methods
        // This avoids storing redundant visibility state on edges
    }

    fn visible_children(&self, entity_id: Uuid) -> Vec<Uuid> {
        self.ownership_edges
            .iter()
            .filter(|e| e.from_entity_id == entity_id && self.edge_visible_ownership(e))
            .map(|e| e.to_entity_id)
            .collect()
    }

    fn visible_parents(&self, entity_id: Uuid) -> Vec<Uuid> {
        self.ownership_edges
            .iter()
            .filter(|e| e.to_entity_id == entity_id && self.edge_visible_ownership(e))
            .map(|e| e.from_entity_id)
            .collect()
    }

    fn visible_controllers(&self, entity_id: Uuid) -> Vec<Uuid> {
        self.control_edges
            .iter()
            .filter(|e| e.controlled_id == entity_id && self.edge_visible_control(e))
            .map(|e| e.controller_id)
            .collect()
    }

    fn visible_controlled(&self, entity_id: Uuid) -> Vec<Uuid> {
        self.control_edges
            .iter()
            .filter(|e| e.controller_id == entity_id && self.edge_visible_control(e))
            .map(|e| e.controlled_id)
            .collect()
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Check if an edge is effective at a given date
fn edge_effective_at(
    effective_from: &Option<NaiveDate>,
    effective_to: &Option<NaiveDate>,
    as_of_date: NaiveDate,
) -> bool {
    // Check effective_from - must be on or before as_of_date
    if let Some(from) = effective_from {
        if *from > as_of_date {
            return false;
        }
    }

    // Check effective_to - must be after as_of_date (or null for current)
    if let Some(to) = effective_to {
        if *to <= as_of_date {
            return false;
        }
    }

    true
}

// =============================================================================
// PATH COMPUTATION (for path_only filter)
// =============================================================================

impl EntityGraph {
    /// Check if entity is on the path from any terminus to the cursor
    fn is_on_path_to_cursor(&self, entity_id: Uuid) -> bool {
        let Some(cursor_id) = self.cursor else {
            return false;
        };

        // BFS from cursor upward to find all ancestors
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(cursor_id);

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current) {
                continue;
            }

            if current == entity_id {
                return true;
            }

            // Add owners (going up the ownership chain)
            if let Some(node) = self.nodes.get(&current) {
                for &owner_id in &node.owners {
                    if !visited.contains(&owner_id) {
                        queue.push_back(owner_id);
                    }
                }
                // Also check controllers
                for &controller_id in &node.controlled_by {
                    if !visited.contains(&controller_id) {
                        queue.push_back(controller_id);
                    }
                }
            }
        }

        false
    }
}

// =============================================================================
// FILTER BUILDER
// =============================================================================

/// Builder for constructing GraphFilters
#[derive(Debug, Clone, Default)]
pub struct FilterBuilder {
    prong: ProngFilter,
    jurisdictions: Option<Vec<String>>,
    fund_types: Option<Vec<String>>,
    entity_types: Option<Vec<EntityType>>,
    as_of_date: Option<NaiveDate>,
    min_ownership_pct: Option<Decimal>,
    path_only: bool,
    same_manco_id: Option<Uuid>,
    same_sicav_id: Option<Uuid>,
}

impl FilterBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prong(mut self, prong: ProngFilter) -> Self {
        self.prong = prong;
        self
    }

    pub fn ownership_only(mut self) -> Self {
        self.prong = ProngFilter::OwnershipOnly;
        self
    }

    pub fn control_only(mut self) -> Self {
        self.prong = ProngFilter::ControlOnly;
        self
    }

    pub fn jurisdictions(mut self, jurisdictions: Vec<String>) -> Self {
        self.jurisdictions = Some(jurisdictions);
        self
    }

    pub fn add_jurisdiction(mut self, jurisdiction: String) -> Self {
        self.jurisdictions
            .get_or_insert_with(Vec::new)
            .push(jurisdiction);
        self
    }

    pub fn fund_types(mut self, fund_types: Vec<String>) -> Self {
        self.fund_types = Some(fund_types);
        self
    }

    pub fn entity_types(mut self, entity_types: Vec<EntityType>) -> Self {
        self.entity_types = Some(entity_types);
        self
    }

    pub fn as_of_date(mut self, date: NaiveDate) -> Self {
        self.as_of_date = Some(date);
        self
    }

    pub fn min_ownership_pct(mut self, pct: Decimal) -> Self {
        self.min_ownership_pct = Some(pct);
        self
    }

    pub fn path_only(mut self, path_only: bool) -> Self {
        self.path_only = path_only;
        self
    }

    pub fn same_manco(mut self, manco_id: Uuid) -> Self {
        self.same_manco_id = Some(manco_id);
        self
    }

    pub fn same_sicav(mut self, sicav_id: Uuid) -> Self {
        self.same_sicav_id = Some(sicav_id);
        self
    }

    pub fn build(self) -> GraphFilters {
        GraphFilters {
            prong: self.prong,
            jurisdictions: self.jurisdictions,
            fund_types: self.fund_types,
            entity_types: self.entity_types,
            as_of_date: self
                .as_of_date
                .unwrap_or_else(|| chrono::Utc::now().date_naive()),
            min_ownership_pct: self.min_ownership_pct,
            path_only: self.path_only,
            same_manco_id: self.same_manco_id,
            same_sicav_id: self.same_sicav_id,
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
    fn test_edge_effective_at() {
        let today = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();

        // Edge with no dates - always effective
        assert!(edge_effective_at(&None, &None, today));

        // Edge starting before today - effective
        let before = NaiveDate::from_ymd_opt(2024, 1, 1);
        assert!(edge_effective_at(&before, &None, today));

        // Edge starting after today - not effective
        let after = NaiveDate::from_ymd_opt(2024, 12, 1);
        assert!(!edge_effective_at(&after, &None, today));

        // Edge ending before today - not effective
        let ended = NaiveDate::from_ymd_opt(2024, 3, 1);
        assert!(!edge_effective_at(&None, &ended, today));

        // Edge with valid range - effective
        let from = NaiveDate::from_ymd_opt(2024, 1, 1);
        let to = NaiveDate::from_ymd_opt(2024, 12, 31);
        assert!(edge_effective_at(&from, &to, today));
    }

    #[test]
    fn test_filter_builder() {
        let filters = FilterBuilder::new()
            .ownership_only()
            .add_jurisdiction("LU".to_string())
            .add_jurisdiction("IE".to_string())
            .min_ownership_pct(Decimal::new(25, 0))
            .build();

        assert!(matches!(filters.prong, ProngFilter::OwnershipOnly));
        assert_eq!(
            filters.jurisdictions,
            Some(vec!["LU".to_string(), "IE".to_string()])
        );
        assert_eq!(filters.min_ownership_pct, Some(Decimal::new(25, 0)));
    }
}
