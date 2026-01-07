//! ScopePath - Hierarchical navigation path through the taxonomy.
//!
//! A ScopePath describes WHERE we are in the navigation hierarchy:
//! - Universe level: `[Universe]`
//! - Book level: `[Universe, Book("UCITS")]`
//! - CBU level: `[Universe, Book("UCITS"), CBU(uuid)]`
//! - Entity level: `[Universe, Book("UCITS"), CBU(uuid), Entity(uuid)]`
//!
//! This is the **location** in the taxonomy tree, distinct from:
//! - ViewState: The visual representation being rendered
//! - TaxonomyStack: The navigation history (back/forward capability)
//! - SessionContext: The full session state including bindings/AST

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// A segment in the scope path - one level in the hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScopeSegment {
    /// Universe level - all CBUs clustered by dimension
    Universe {
        /// Clustering dimension (jurisdiction, client_type, risk_rating, product)
        cluster_by: String,
    },

    /// Book level - a single cluster/grouping
    Book {
        /// Book identifier (e.g., "UCITS", "LU", "HIGH_RISK")
        book_id: String,
        /// Display label
        label: String,
    },

    /// CBU level - a specific Client Business Unit
    Cbu { cbu_id: Uuid, name: String },

    /// Entity level - a specific entity within a CBU
    Entity {
        entity_id: Uuid,
        name: String,
        entity_type: String,
    },

    /// Type grouping - viewing entities of a specific type
    TypeGroup { type_code: String, label: String },

    /// Custom zoom level - for fractal navigation into arbitrary nodes
    Custom {
        node_id: Uuid,
        label: String,
        node_type: String,
    },
}

impl ScopeSegment {
    /// Get the primary identifier for this segment
    pub fn id(&self) -> String {
        match self {
            Self::Universe { cluster_by } => format!("universe:{}", cluster_by),
            Self::Book { book_id, .. } => format!("book:{}", book_id),
            Self::Cbu { cbu_id, .. } => format!("cbu:{}", cbu_id),
            Self::Entity { entity_id, .. } => format!("entity:{}", entity_id),
            Self::TypeGroup { type_code, .. } => format!("type:{}", type_code),
            Self::Custom { node_id, .. } => format!("custom:{}", node_id),
        }
    }

    /// Get the display label for this segment
    pub fn label(&self) -> &str {
        match self {
            Self::Universe { cluster_by } => cluster_by,
            Self::Book { label, .. } => label,
            Self::Cbu { name, .. } => name,
            Self::Entity { name, .. } => name,
            Self::TypeGroup { label, .. } => label,
            Self::Custom { label, .. } => label,
        }
    }

    /// Get the segment type name
    pub fn segment_type(&self) -> &'static str {
        match self {
            Self::Universe { .. } => "universe",
            Self::Book { .. } => "book",
            Self::Cbu { .. } => "cbu",
            Self::Entity { .. } => "entity",
            Self::TypeGroup { .. } => "type_group",
            Self::Custom { .. } => "custom",
        }
    }

    /// Check if this segment represents a CBU
    pub fn is_cbu(&self) -> bool {
        matches!(self, Self::Cbu { .. })
    }

    /// Get CBU ID if this is a CBU segment
    pub fn cbu_id(&self) -> Option<Uuid> {
        match self {
            Self::Cbu { cbu_id, .. } => Some(*cbu_id),
            _ => None,
        }
    }

    /// Get entity ID if this is an entity segment
    pub fn entity_id(&self) -> Option<Uuid> {
        match self {
            Self::Entity { entity_id, .. } => Some(*entity_id),
            _ => None,
        }
    }
}

impl fmt::Display for ScopeSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A complete path through the scope hierarchy.
///
/// The path always starts with Universe and can extend to deeper levels.
/// This is analogous to a file system path but for the taxonomy tree.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopePath {
    /// Ordered list of segments from root (Universe) to current position
    segments: Vec<ScopeSegment>,
}

impl ScopePath {
    /// Create an empty path (no scope set)
    pub fn empty() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Create a path starting at universe level
    pub fn universe(cluster_by: impl Into<String>) -> Self {
        Self {
            segments: vec![ScopeSegment::Universe {
                cluster_by: cluster_by.into(),
            }],
        }
    }

    /// Create a path to a specific book
    pub fn book(
        cluster_by: impl Into<String>,
        book_id: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            segments: vec![
                ScopeSegment::Universe {
                    cluster_by: cluster_by.into(),
                },
                ScopeSegment::Book {
                    book_id: book_id.into(),
                    label: label.into(),
                },
            ],
        }
    }

    /// Create a path directly to a CBU (common case)
    pub fn cbu(cbu_id: Uuid, name: impl Into<String>) -> Self {
        Self {
            segments: vec![ScopeSegment::Cbu {
                cbu_id,
                name: name.into(),
            }],
        }
    }

    /// Create a path to an entity within a CBU
    pub fn entity(
        cbu_id: Uuid,
        cbu_name: impl Into<String>,
        entity_id: Uuid,
        entity_name: impl Into<String>,
        entity_type: impl Into<String>,
    ) -> Self {
        Self {
            segments: vec![
                ScopeSegment::Cbu {
                    cbu_id,
                    name: cbu_name.into(),
                },
                ScopeSegment::Entity {
                    entity_id,
                    name: entity_name.into(),
                    entity_type: entity_type.into(),
                },
            ],
        }
    }

    /// Push a new segment onto the path
    pub fn push(&mut self, segment: ScopeSegment) {
        self.segments.push(segment);
    }

    /// Pop the last segment from the path (zoom out)
    pub fn pop(&mut self) -> Option<ScopeSegment> {
        self.segments.pop()
    }

    /// Get the current (deepest) segment
    pub fn current(&self) -> Option<&ScopeSegment> {
        self.segments.last()
    }

    /// Get the parent segment (one level up)
    pub fn parent(&self) -> Option<&ScopeSegment> {
        if self.segments.len() >= 2 {
            self.segments.get(self.segments.len() - 2)
        } else {
            None
        }
    }

    /// Get all segments
    pub fn segments(&self) -> &[ScopeSegment] {
        &self.segments
    }

    /// Get the depth of the path (0 = empty, 1 = universe, etc.)
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Check if path is empty
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Check if at universe level
    pub fn is_universe(&self) -> bool {
        self.depth() == 1 && matches!(self.current(), Some(ScopeSegment::Universe { .. }))
    }

    /// Check if at book level
    pub fn is_book(&self) -> bool {
        matches!(self.current(), Some(ScopeSegment::Book { .. }))
    }

    /// Check if at CBU level
    pub fn is_cbu(&self) -> bool {
        matches!(self.current(), Some(ScopeSegment::Cbu { .. }))
    }

    /// Check if at entity level
    pub fn is_entity(&self) -> bool {
        matches!(self.current(), Some(ScopeSegment::Entity { .. }))
    }

    /// Get the CBU ID from the path (if any segment is a CBU)
    pub fn cbu_id(&self) -> Option<Uuid> {
        self.segments.iter().find_map(|s| s.cbu_id())
    }

    /// Get the entity ID if at entity level
    pub fn entity_id(&self) -> Option<Uuid> {
        self.current().and_then(|s| s.entity_id())
    }

    /// Get breadcrumb labels for display
    pub fn breadcrumbs(&self) -> Vec<&str> {
        self.segments.iter().map(|s| s.label()).collect()
    }

    /// Get breadcrumb with segment types
    pub fn breadcrumbs_typed(&self) -> Vec<(&'static str, &str)> {
        self.segments
            .iter()
            .map(|s| (s.segment_type(), s.label()))
            .collect()
    }

    /// Truncate path to a specific depth (for back_to navigation)
    pub fn truncate(&mut self, depth: usize) {
        self.segments.truncate(depth);
    }

    /// Clone and extend with a new segment
    pub fn extend(&self, segment: ScopeSegment) -> Self {
        let mut new_path = self.clone();
        new_path.push(segment);
        new_path
    }

    /// Convert to a path string (for display/debugging)
    pub fn to_path_string(&self) -> String {
        if self.is_empty() {
            return "/".to_string();
        }
        format!("/{}", self.breadcrumbs().join("/"))
    }
}

impl fmt::Display for ScopePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_path_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_path() {
        let path = ScopePath::empty();
        assert!(path.is_empty());
        assert_eq!(path.depth(), 0);
        assert_eq!(path.to_path_string(), "/");
    }

    #[test]
    fn test_universe_path() {
        let path = ScopePath::universe("jurisdiction");
        assert!(path.is_universe());
        assert_eq!(path.depth(), 1);
        assert_eq!(path.to_path_string(), "/jurisdiction");
    }

    #[test]
    fn test_cbu_path() {
        let cbu_id = Uuid::new_v4();
        let path = ScopePath::cbu(cbu_id, "Apex Fund");
        assert!(path.is_cbu());
        assert_eq!(path.cbu_id(), Some(cbu_id));
        assert_eq!(path.to_path_string(), "/Apex Fund");
    }

    #[test]
    fn test_entity_path() {
        let cbu_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();
        let path = ScopePath::entity(
            cbu_id,
            "Apex Fund",
            entity_id,
            "John Smith",
            "proper_person",
        );
        assert!(path.is_entity());
        assert_eq!(path.cbu_id(), Some(cbu_id));
        assert_eq!(path.entity_id(), Some(entity_id));
        assert_eq!(path.to_path_string(), "/Apex Fund/John Smith");
    }

    #[test]
    fn test_path_navigation() {
        let cbu_id = Uuid::new_v4();
        let mut path = ScopePath::cbu(cbu_id, "Apex Fund");

        // Zoom in
        let entity_id = Uuid::new_v4();
        path.push(ScopeSegment::Entity {
            entity_id,
            name: "John Smith".to_string(),
            entity_type: "proper_person".to_string(),
        });
        assert!(path.is_entity());
        assert_eq!(path.depth(), 2);

        // Zoom out
        path.pop();
        assert!(path.is_cbu());
        assert_eq!(path.depth(), 1);
    }

    #[test]
    fn test_breadcrumbs() {
        let path = ScopePath::book("jurisdiction", "LU", "Luxembourg");
        let crumbs = path.breadcrumbs();
        assert_eq!(crumbs, vec!["jurisdiction", "Luxembourg"]);
    }
}
