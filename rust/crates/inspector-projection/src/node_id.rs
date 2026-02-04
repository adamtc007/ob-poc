//! NodeId - Stable path-based identifiers for projection nodes.
//!
//! Format: `{kind}:{qualifier}[:{subpath}]`
//!
//! Examples:
//! - `cbu:allianz-ie-funds`
//! - `entity:uuid:fund_001`
//! - `matrix:focus:mic:XLON` (uppercase MIC preserved)
//! - `register:control:edge:001`
//!
//! The kind prefix (before first `:`) is lowercase only.
//! Subsequent segments allow uppercase to preserve domain values like MIC codes.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::Hash;
use std::sync::OnceLock;

/// Pattern for valid NodeIds.
/// - Kind prefix: lowercase letters and hyphens (e.g., matrix-slice, holding-edge)
/// - After first colon: letters, digits, underscore, colon, hyphen
fn node_id_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[a-z][a-z\-]*:[A-Za-z0-9_:\-]+$").unwrap())
}

/// Error when parsing an invalid NodeId.
#[derive(Debug, Clone, thiserror::Error)]
pub enum NodeIdError {
    #[error("Invalid NodeId format: '{0}'. Expected pattern: kind:qualifier[:subpath]")]
    InvalidFormat(String),

    #[error("NodeId cannot be empty")]
    Empty,
}

/// Stable, path-based node identifier for deterministic addressing.
///
/// NodeIds are immutable strings that uniquely identify nodes in a projection.
/// They follow a hierarchical format that enables:
/// - Stable IDs across re-renders
/// - O(1) kind lookup (prefix before first colon)
/// - Natural path-based navigation
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NodeId(String);

impl NodeId {
    /// Create a new NodeId, validating the format.
    ///
    /// # Errors
    /// Returns `NodeIdError` if the format is invalid.
    pub fn new(s: impl Into<String>) -> Result<Self, NodeIdError> {
        let s = s.into();
        if s.is_empty() {
            return Err(NodeIdError::Empty);
        }
        if !node_id_pattern().is_match(&s) {
            return Err(NodeIdError::InvalidFormat(s));
        }
        Ok(Self(s))
    }

    /// Create a NodeId from kind and qualifier segments.
    ///
    /// # Panics
    /// Panics if the resulting ID is invalid (should not happen with valid inputs).
    pub fn from_parts(kind: &str, qualifier: &str) -> Self {
        let s = format!("{}:{}", kind, qualifier);
        Self::new(s).expect("Invalid NodeId from parts")
    }

    /// Create a child NodeId by appending a segment.
    pub fn child(&self, segment: &str) -> Self {
        let s = format!("{}:{}", self.0, segment);
        // Child should always be valid if parent is valid
        Self::new(&s).unwrap_or(Self(s))
    }

    /// Get the kind prefix (before first colon).
    pub fn kind(&self) -> &str {
        self.0.split(':').next().unwrap_or("")
    }

    /// Get the full ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the segments after the kind prefix.
    pub fn qualifier(&self) -> &str {
        self.0.find(':').map(|i| &self.0[i + 1..]).unwrap_or("")
    }

    /// Get all segments as an iterator.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split(':')
    }

    /// Get the depth (number of colons).
    pub fn depth(&self) -> usize {
        self.0.matches(':').count()
    }

    /// Get parent NodeId (remove last segment), if any.
    pub fn parent(&self) -> Option<Self> {
        let last_colon = self.0.rfind(':')?;
        if last_colon == self.0.find(':')? {
            // Only one colon - no parent beyond kind:qualifier
            return None;
        }
        Some(Self(self.0[..last_colon].to_string()))
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId(\"{}\")", self.0)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<NodeId> for String {
    fn from(id: NodeId) -> Self {
        id.0
    }
}

impl TryFrom<String> for NodeId {
    type Error = NodeIdError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        NodeId::new(s)
    }
}

impl TryFrom<&str> for NodeId {
    type Error = NodeIdError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        NodeId::new(s)
    }
}

impl PartialOrd for NodeId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NodeId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_node_ids() {
        // Basic formats
        assert!(NodeId::new("cbu:0").is_ok());
        assert!(NodeId::new("cbu:allianz-ie-funds").is_ok());
        assert!(NodeId::new("entity:uuid:fund_001").is_ok());

        // Uppercase after prefix (MIC codes, UUIDs)
        assert!(NodeId::new("matrix:focus:mic:XLON").is_ok());
        assert!(NodeId::new("matrix:focus:mic:XEUR").is_ok());

        // Hyphens in IDs
        assert!(NodeId::new("cbu:allianz-ie").is_ok());
        assert!(NodeId::new("register:control:edge-001").is_ok());

        // Deep paths
        assert!(NodeId::new("cbu:members:0:page:2").is_ok());
    }

    #[test]
    fn test_invalid_node_ids() {
        // Empty
        assert!(NodeId::new("").is_err());

        // No colon
        assert!(NodeId::new("cbu").is_err());

        // Uppercase in kind prefix
        assert!(NodeId::new("CBU:test").is_err());
        assert!(NodeId::new("Cbu:test").is_err());

        // Invalid characters
        assert!(NodeId::new("cbu:test space").is_err());
        assert!(NodeId::new("cbu:test@value").is_err());
    }

    #[test]
    fn test_kind_extraction() {
        let id = NodeId::new("matrix:focus:mic:XLON").unwrap();
        assert_eq!(id.kind(), "matrix");

        let id = NodeId::new("cbu:allianz").unwrap();
        assert_eq!(id.kind(), "cbu");
    }

    #[test]
    fn test_qualifier_extraction() {
        let id = NodeId::new("matrix:focus:mic:XLON").unwrap();
        assert_eq!(id.qualifier(), "focus:mic:XLON");

        let id = NodeId::new("cbu:allianz").unwrap();
        assert_eq!(id.qualifier(), "allianz");
    }

    #[test]
    fn test_child_creation() {
        let parent = NodeId::new("cbu:allianz").unwrap();
        let child = parent.child("members");
        assert_eq!(child.as_str(), "cbu:allianz:members");

        let grandchild = child.child("page:1");
        assert_eq!(grandchild.as_str(), "cbu:allianz:members:page:1");
    }

    #[test]
    fn test_parent_extraction() {
        let id = NodeId::new("cbu:allianz:members:page:1").unwrap();

        let parent = id.parent().unwrap();
        assert_eq!(parent.as_str(), "cbu:allianz:members:page");

        let grandparent = parent.parent().unwrap();
        assert_eq!(grandparent.as_str(), "cbu:allianz:members");

        // Eventually reaches None
        let root = NodeId::new("cbu:allianz").unwrap();
        assert!(root.parent().is_none());
    }

    #[test]
    fn test_depth() {
        assert_eq!(NodeId::new("cbu:0").unwrap().depth(), 1);
        assert_eq!(NodeId::new("cbu:allianz:members").unwrap().depth(), 2);
        assert_eq!(NodeId::new("matrix:focus:mic:XLON").unwrap().depth(), 3);
    }

    #[test]
    fn test_serde_roundtrip() {
        let id = NodeId::new("matrix:focus:mic:XLON").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"matrix:focus:mic:XLON\"");

        let parsed: NodeId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn test_serde_invalid_rejects() {
        let result: Result<NodeId, _> = serde_json::from_str("\"INVALID:test\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_ordering() {
        let a = NodeId::new("cbu:a").unwrap();
        let b = NodeId::new("cbu:b").unwrap();
        let c = NodeId::new("entity:a").unwrap();

        assert!(a < b);
        assert!(b < c); // 'c' < 'e' lexicographically
    }
}
