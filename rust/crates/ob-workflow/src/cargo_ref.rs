//! Cargo Reference URI Scheme
//!
//! Typed enum for cargo references to prevent string parsing bugs.
//! All cargo in the task queue system is a POINTER (URI) to actual data stored elsewhere.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// URI scheme for cargo references.
/// Typed enum prevents string parsing bugs and provides compile-time guarantees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CargoRef {
    /// Document entity: `document://ob-poc/{document_id}`
    Document { schema: String, id: Uuid },

    /// Document version (preferred for callbacks): `version://ob-poc/{version_id}`
    Version { schema: String, id: Uuid },

    /// Entity (for entity-creation tasks): `entity://ob-poc/{entity_id}`
    Entity { schema: String, id: Uuid },

    /// Screening result: `screening://ob-poc/{screening_id}`
    Screening { schema: String, id: Uuid },

    /// External system passthrough: `external://{system}/{external_id}`
    External { system: String, id: String },
}

/// Errors that can occur when parsing a cargo reference URI
#[derive(Debug, thiserror::Error)]
pub enum CargoRefParseError {
    #[error("Invalid URI format: expected 'scheme://path'")]
    InvalidFormat,

    #[error("Unknown scheme: {0}")]
    UnknownScheme(String),

    #[error("Invalid UUID in path: {0}")]
    InvalidUuid(#[from] uuid::Error),

    #[error("Missing path component after scheme")]
    MissingPath,
}

impl CargoRef {
    /// Create a version reference with default schema
    pub fn version(id: Uuid) -> Self {
        Self::Version {
            schema: "ob-poc".into(),
            id,
        }
    }

    /// Create a document reference with default schema
    pub fn document(id: Uuid) -> Self {
        Self::Document {
            schema: "ob-poc".into(),
            id,
        }
    }

    /// Create an entity reference with default schema
    pub fn entity(id: Uuid) -> Self {
        Self::Entity {
            schema: "ob-poc".into(),
            id,
        }
    }

    /// Create a screening reference with default schema
    pub fn screening(id: Uuid) -> Self {
        Self::Screening {
            schema: "ob-poc".into(),
            id,
        }
    }

    /// Create an external reference
    pub fn external(system: impl Into<String>, id: impl Into<String>) -> Self {
        Self::External {
            system: system.into(),
            id: id.into(),
        }
    }

    /// Parse a cargo reference from a URI string
    pub fn parse(s: &str) -> Result<Self, CargoRefParseError> {
        let (scheme, rest) = s
            .split_once("://")
            .ok_or(CargoRefParseError::InvalidFormat)?;

        match scheme {
            "version" => {
                let (schema, id) = parse_schema_id(rest)?;
                Ok(Self::Version { schema, id })
            }
            "document" => {
                let (schema, id) = parse_schema_id(rest)?;
                Ok(Self::Document { schema, id })
            }
            "entity" => {
                let (schema, id) = parse_schema_id(rest)?;
                Ok(Self::Entity { schema, id })
            }
            "screening" => {
                let (schema, id) = parse_schema_id(rest)?;
                Ok(Self::Screening { schema, id })
            }
            "external" => {
                let (system, id) = rest
                    .split_once('/')
                    .ok_or(CargoRefParseError::MissingPath)?;
                Ok(Self::External {
                    system: system.to_string(),
                    id: id.to_string(),
                })
            }
            _ => Err(CargoRefParseError::UnknownScheme(scheme.to_string())),
        }
    }

    /// Convert to URI string
    pub fn to_uri(&self) -> String {
        match self {
            Self::Version { schema, id } => format!("version://{}/{}", schema, id),
            Self::Document { schema, id } => format!("document://{}/{}", schema, id),
            Self::Entity { schema, id } => format!("entity://{}/{}", schema, id),
            Self::Screening { schema, id } => format!("screening://{}/{}", schema, id),
            Self::External { system, id } => format!("external://{}/{}", system, id),
        }
    }

    /// Get the scheme of this cargo reference
    pub fn scheme(&self) -> &'static str {
        match self {
            Self::Version { .. } => "version",
            Self::Document { .. } => "document",
            Self::Entity { .. } => "entity",
            Self::Screening { .. } => "screening",
            Self::External { .. } => "external",
        }
    }

    /// Get the UUID if this is an internal reference (not external)
    pub fn uuid(&self) -> Option<Uuid> {
        match self {
            Self::Version { id, .. }
            | Self::Document { id, .. }
            | Self::Entity { id, .. }
            | Self::Screening { id, .. } => Some(*id),
            Self::External { .. } => None,
        }
    }
}

/// Parse "schema/uuid" format
fn parse_schema_id(s: &str) -> Result<(String, Uuid), CargoRefParseError> {
    let (schema, id_str) = s.split_once('/').ok_or(CargoRefParseError::MissingPath)?;
    let id = Uuid::parse_str(id_str)?;
    Ok((schema.to_string(), id))
}

impl std::fmt::Display for CargoRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_uri())
    }
}

impl std::str::FromStr for CargoRef {
    type Err = CargoRefParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_roundtrip() {
        let id = Uuid::new_v4();
        let cargo = CargoRef::version(id);
        let uri = cargo.to_uri();
        let parsed = CargoRef::parse(&uri).unwrap();
        assert_eq!(cargo, parsed);
    }

    #[test]
    fn test_document_roundtrip() {
        let id = Uuid::new_v4();
        let cargo = CargoRef::document(id);
        let uri = cargo.to_uri();
        let parsed = CargoRef::parse(&uri).unwrap();
        assert_eq!(cargo, parsed);
    }

    #[test]
    fn test_external_roundtrip() {
        let cargo = CargoRef::external("camunda", "process-12345");
        let uri = cargo.to_uri();
        assert_eq!(uri, "external://camunda/process-12345");
        let parsed = CargoRef::parse(&uri).unwrap();
        assert_eq!(cargo, parsed);
    }

    #[test]
    fn test_invalid_format() {
        assert!(CargoRef::parse("not-a-uri").is_err());
        assert!(CargoRef::parse("unknown://schema/id").is_err());
    }

    #[test]
    fn test_uuid_extraction() {
        let id = Uuid::new_v4();
        let cargo = CargoRef::version(id);
        assert_eq!(cargo.uuid(), Some(id));

        let external = CargoRef::external("sys", "id");
        assert_eq!(external.uuid(), None);
    }
}
