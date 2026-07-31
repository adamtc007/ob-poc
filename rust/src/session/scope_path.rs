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
pub(crate) enum ScopeSegment {
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
    /// Get the display label for this segment
    pub(crate) fn label(&self) -> &str {
        match self {
            Self::Universe { cluster_by } => cluster_by,
            Self::Book { label, .. } => label,
            Self::Cbu { name, .. } => name,
            Self::Entity { name, .. } => name,
            Self::TypeGroup { label, .. } => label,
            Self::Custom { label, .. } => label,
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
    /// Check if path is empty
    pub(crate) fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Get breadcrumb labels for display
    pub(crate) fn breadcrumbs(&self) -> Vec<&str> {
        self.segments.iter().map(|s| s.label()).collect()
    }

    /// Convert to a path string (for display/debugging)
    pub(crate) fn to_path_string(&self) -> String {
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
