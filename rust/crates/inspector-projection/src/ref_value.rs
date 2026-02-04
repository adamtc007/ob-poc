//! RefValue - `$ref` linking for node-to-node relationships.
//!
//! All node-to-node relationships in the projection use `$ref` strings,
//! enabling a flat node map with random access by NodeId.
//!
//! Serializes as: `{ "$ref": "node_id" }`

use crate::node_id::NodeId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Reference to another node in the projection.
///
/// This is the wire format for all node-to-node links.
/// Using `$ref` instead of embedding enables:
/// - Flat node map for O(1) lookup
/// - No recursive descent required
/// - Easy cycle detection
/// - Deterministic serialization
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RefValue {
    /// The target node ID.
    #[serde(rename = "$ref")]
    pub target: NodeId,
}

impl RefValue {
    /// Create a new reference to a node.
    pub fn new(target: NodeId) -> Self {
        Self { target }
    }

    /// Create a reference from a string, parsing as NodeId.
    ///
    /// # Errors
    /// Returns error if the string is not a valid NodeId.
    pub fn parse(s: &str) -> Result<Self, crate::node_id::NodeIdError> {
        Ok(Self {
            target: NodeId::new(s)?,
        })
    }

    /// Get the target NodeId.
    pub fn target(&self) -> &NodeId {
        &self.target
    }
}

impl fmt::Debug for RefValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RefValue({})", self.target)
    }
}

impl fmt::Display for RefValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "$ref:{}", self.target)
    }
}

impl From<NodeId> for RefValue {
    fn from(id: NodeId) -> Self {
        Self::new(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ref_value_creation() {
        let id = NodeId::new("entity:fund_001").unwrap();
        let ref_val = RefValue::new(id.clone());
        assert_eq!(ref_val.target(), &id);
    }

    #[test]
    fn test_ref_value_parse() {
        let ref_val = RefValue::parse("cbu:allianz").unwrap();
        assert_eq!(ref_val.target().as_str(), "cbu:allianz");
    }

    #[test]
    fn test_ref_value_serialization() {
        let ref_val = RefValue::parse("entity:fund_001").unwrap();
        let json = serde_json::to_string(&ref_val).unwrap();
        assert_eq!(json, r#"{"$ref":"entity:fund_001"}"#);
    }

    #[test]
    fn test_ref_value_deserialization() {
        let json = r#"{"$ref":"matrix:focus:mic:XLON"}"#;
        let ref_val: RefValue = serde_json::from_str(json).unwrap();
        assert_eq!(ref_val.target().as_str(), "matrix:focus:mic:XLON");
    }

    #[test]
    fn test_ref_value_yaml_roundtrip() {
        let ref_val = RefValue::parse("cbu:allianz:members").unwrap();
        let yaml = serde_yaml::to_string(&ref_val).unwrap();
        let parsed: RefValue = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed, ref_val);
    }

    #[test]
    fn test_ref_value_invalid_target() {
        let result = RefValue::parse("INVALID");
        assert!(result.is_err());
    }
}
