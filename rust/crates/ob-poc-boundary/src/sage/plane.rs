//! ObservationPlane — the lens through which the user is viewing the system.
//!
//! Instance: Operating on entity instances identified by UUID (default).
//! Structure: Operating on entity types, schemas, taxonomies (data management).
//! Registry: Operating on the semantic registry / governance metadata.

use serde::{Deserialize, Serialize};

/// The observation plane determines which slice of the verb space is relevant.
///
/// This is the single most important classification signal for narrowing
/// 1,263 verbs down to ~100-300 candidates before any scoring begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObservationPlane {
    /// Operating on entity instances identified by UUID.
    /// Default plane. Covers: cbu.create, entity.update, session.load-galaxy, etc.
    Instance,

    /// Operating on entity types, schemas, taxonomies.
    /// Triggered by semos-data-management / semos-data stage focus with no instance targeting.
    /// Covers: schema.entity.describe, schema.entity.list-attributes, registry.discover-dsl, etc.
    Structure,

    /// Operating on the semantic registry / governance metadata.
    /// Triggered by semos-stewardship stage focus.
    /// Covers: changeset.*, governance.*, focus.*, audit.* verbs.
    Registry,
}

impl ObservationPlane {
    /// Returns the canonical string key for logging and telemetry.
    pub fn as_str(&self) -> &'static str {
        match self {
            ObservationPlane::Instance => "instance",
            ObservationPlane::Structure => "structure",
            ObservationPlane::Registry => "registry",
        }
    }
}

impl std::fmt::Display for ObservationPlane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plane_display() {
        assert_eq!(ObservationPlane::Instance.to_string(), "instance");
        assert_eq!(ObservationPlane::Structure.to_string(), "structure");
        assert_eq!(ObservationPlane::Registry.to_string(), "registry");
    }

    #[test]
    fn test_plane_serde_roundtrip() {
        let plane = ObservationPlane::Structure;
        let json = serde_json::to_string(&plane).unwrap();
        let back: ObservationPlane = serde_json::from_str(&json).unwrap();
        assert_eq!(plane, back);
    }
}
