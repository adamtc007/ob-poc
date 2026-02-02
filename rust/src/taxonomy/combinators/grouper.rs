//! Groupers for Taxonomy Construction
//!
//! Groupers create intermediate nodes that cluster source items by some criterion.
//! For example, grouping CBUs by jurisdiction creates jurisdiction nodes as parents.

use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use uuid::Uuid;

use super::source::SourceItem;
use crate::taxonomy::types::{DimensionValues, NodeType};

/// Generate a deterministic UUID from a string (like uuid v5 but simpler)
fn uuid_from_string(s: &str) -> Uuid {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    let hash = hasher.finish();
    // Use the hash to create a UUID-like value
    let bytes: [u8; 16] = {
        let mut b = [0u8; 16];
        b[0..8].copy_from_slice(&hash.to_le_bytes());
        b[8..16].copy_from_slice(&hash.to_be_bytes());
        b
    };
    Uuid::from_bytes(bytes)
}

/// Result of grouping - a group node with its member items
#[derive(Debug, Clone)]
pub struct GroupResult {
    /// ID for the group node (generated)
    pub group_id: Uuid,
    /// Label for the group
    pub label: String,
    /// Node type for the group
    pub node_type: NodeType,
    /// Dimensions for the group (aggregated from members)
    pub dimensions: DimensionValues,
    /// Items belonging to this group
    pub members: Vec<SourceItem>,
}

/// Trait for creating groupings of source items
pub trait Grouper: Send + Sync + Debug {
    /// Group items into named buckets
    fn group(&self, items: Vec<SourceItem>) -> Vec<GroupResult>;

    /// Clone into boxed trait object
    fn clone_box(&self) -> Box<dyn Grouper>;
}

/// Boxed grouper for ergonomics
pub type GrouperBox = Box<dyn Grouper>;

/// Group by a specific field in dimensions
#[derive(Debug, Clone)]
pub struct FieldGrouper {
    field: String,
    node_type: NodeType,
}

impl FieldGrouper {
    pub fn new(field: impl Into<String>, node_type: NodeType) -> Self {
        Self {
            field: field.into(),
            node_type,
        }
    }
}

impl Grouper for FieldGrouper {
    fn group(&self, items: Vec<SourceItem>) -> Vec<GroupResult> {
        let mut groups: HashMap<String, Vec<SourceItem>> = HashMap::new();

        for item in items {
            let key = item
                .dimensions
                .get(&self.field)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            groups.entry(key).or_default().push(item);
        }

        groups
            .into_iter()
            .map(|(label, members)| {
                // Generate deterministic UUID from label
                let group_id = uuid_from_string(&label);
                GroupResult {
                    group_id,
                    label,
                    node_type: self.node_type,
                    dimensions: DimensionValues::default(),
                    members,
                }
            })
            .collect()
    }

    fn clone_box(&self) -> Box<dyn Grouper> {
        Box::new(self.clone())
    }
}

/// Group by jurisdiction
#[derive(Debug, Clone, Default)]
pub struct JurisdictionGrouper;

impl JurisdictionGrouper {
    pub fn new() -> Self {
        Self
    }
}

impl Grouper for JurisdictionGrouper {
    fn group(&self, items: Vec<SourceItem>) -> Vec<GroupResult> {
        FieldGrouper::new("jurisdiction", NodeType::Cluster).group(items)
    }

    fn clone_box(&self) -> Box<dyn Grouper> {
        Box::new(self.clone())
    }
}

/// Group by entity type
#[derive(Debug, Clone, Default)]
pub struct EntityTypeGrouper;

impl EntityTypeGrouper {
    pub fn new() -> Self {
        Self
    }
}

impl Grouper for EntityTypeGrouper {
    fn group(&self, items: Vec<SourceItem>) -> Vec<GroupResult> {
        FieldGrouper::new("entity_type", NodeType::Cluster).group(items)
    }

    fn clone_box(&self) -> Box<dyn Grouper> {
        Box::new(self.clone())
    }
}

/// No grouping - items become direct children
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct NoGrouper;

impl Grouper for NoGrouper {
    fn group(&self, items: Vec<SourceItem>) -> Vec<GroupResult> {
        // Each item becomes its own "group" of one
        items
            .into_iter()
            .map(|item| GroupResult {
                group_id: item.id,
                label: item.label.clone(),
                node_type: NodeType::Entity, // Default, will be overridden
                dimensions: item.dimensions.clone(),
                members: vec![item],
            })
            .collect()
    }

    fn clone_box(&self) -> Box<dyn Grouper> {
        Box::new(self.clone())
    }
}

/// Composite grouper - applies multiple groupers in sequence
#[allow(dead_code)]
#[derive(Debug)]
pub struct CompositeGrouper {
    groupers: Vec<Box<dyn Grouper>>,
}

impl Clone for CompositeGrouper {
    fn clone(&self) -> Self {
        Self {
            groupers: self.groupers.iter().map(|g| g.clone_box()).collect(),
        }
    }
}

impl CompositeGrouper {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            groupers: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn then(mut self, grouper: impl Grouper + 'static) -> Self {
        self.groupers.push(Box::new(grouper));
        self
    }
}

impl Default for CompositeGrouper {
    fn default() -> Self {
        Self::new()
    }
}

impl Grouper for CompositeGrouper {
    fn group(&self, items: Vec<SourceItem>) -> Vec<GroupResult> {
        if self.groupers.is_empty() {
            return NoGrouper.group(items);
        }

        // Apply first grouper
        let mut results = self.groupers[0].group(items);

        // Apply subsequent groupers to members of each group
        for grouper in &self.groupers[1..] {
            results = results
                .into_iter()
                .flat_map(|group| grouper.group(group.members))
                .collect();
        }

        results
    }

    fn clone_box(&self) -> Box<dyn Grouper> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_items() -> Vec<SourceItem> {
        vec![
            {
                let mut dims = DimensionValues::default();
                dims.set("jurisdiction", "LU");
                dims.set("entity_type", "FUND");
                SourceItem::new(Uuid::now_v7(), "Fund A", dims)
            },
            {
                let mut dims = DimensionValues::default();
                dims.set("jurisdiction", "LU");
                dims.set("entity_type", "FUND");
                SourceItem::new(Uuid::now_v7(), "Fund B", dims)
            },
            {
                let mut dims = DimensionValues::default();
                dims.set("jurisdiction", "IE");
                dims.set("entity_type", "CORPORATE");
                SourceItem::new(Uuid::now_v7(), "Corp C", dims)
            },
        ]
    }

    #[test]
    fn test_jurisdiction_grouper() {
        let items = make_items();
        let grouper = JurisdictionGrouper::new();
        let groups = grouper.group(items);

        assert_eq!(groups.len(), 2); // LU and IE
        let lu_group = groups.iter().find(|g| g.label == "LU").unwrap();
        assert_eq!(lu_group.members.len(), 2);
    }

    #[test]
    fn test_no_grouper() {
        let items = make_items();
        let grouper = NoGrouper;
        let groups = grouper.group(items);

        assert_eq!(groups.len(), 3); // Each item is its own group
    }
}
