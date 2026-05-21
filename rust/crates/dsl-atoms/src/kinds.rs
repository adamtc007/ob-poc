//! Atom kind taxonomy for the unified DSL v0.1.
//!
//! Defines the closed catalogue of structural kinds (20 variants) and
//! declarative kinds (4 variants), plus the classification function
//! `classify(kind_str)` that maps atom kind strings to typed enum values.

use serde::{Deserialize, Serialize};

/// The 20 structural atom kinds. The structural kind catalogue is closed in v0.1.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StructuralKind {
    Verb,
    Invoke,
    Node,
    Gateway,
    Flow,
    BoundaryAttachment,
    ParallelJoin,
    Entity,
    Relationship,
    Predicate,
    Decision,
    DataType,
    MessageDefinition,
    TimerDefinition,
    ErrorDefinition,
    GraphPack,
    UtteranceBinding,
    ConstellationRoot,
    WorkspaceConstraint,
    DecisionPack,
}

/// The 4 declarative atom kinds. These carry governance and provenance metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeclarativeKind {
    Provenance,
    GovernanceStatus,
    ReviewAnnotation,
    JurisdictionTag,
}

/// Classified atom kind. Every atom has exactly one kind class.
///
/// Unknown strings are treated as `UnknownStructural` in v0.1 because the
/// structural kind catalogue is closed (parse errors should surface these).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtomKindClass {
    Structural(StructuralKind),
    Declarative(DeclarativeKind),
    /// A kind string that looks declarative but is not in the known catalogue.
    UnknownDeclarative(String),
    /// A kind string that is not in any known catalogue. In v0.1 all unknown
    /// kinds are represented as `UnknownStructural` because the structural
    /// catalogue is closed and unknown kinds should produce parse diagnostics.
    UnknownStructural(String),
}

/// Classify an atom kind string into a typed [`AtomKindClass`].
///
/// The mapping is exact-string only — no fuzzy matching or case folding.
/// Any string not listed in the known catalogues is classified as
/// [`AtomKindClass::UnknownStructural`].
pub fn classify(kind_str: &str) -> AtomKindClass {
    match kind_str {
        // Structural kinds
        "verb" => AtomKindClass::Structural(StructuralKind::Verb),
        "invoke" => AtomKindClass::Structural(StructuralKind::Invoke),
        "node" => AtomKindClass::Structural(StructuralKind::Node),
        "gateway" => AtomKindClass::Structural(StructuralKind::Gateway),
        "flow" => AtomKindClass::Structural(StructuralKind::Flow),
        "boundary-attachment" => AtomKindClass::Structural(StructuralKind::BoundaryAttachment),
        "parallel-join" => AtomKindClass::Structural(StructuralKind::ParallelJoin),
        "entity" => AtomKindClass::Structural(StructuralKind::Entity),
        "relationship" => AtomKindClass::Structural(StructuralKind::Relationship),
        "predicate" => AtomKindClass::Structural(StructuralKind::Predicate),
        "decision" => AtomKindClass::Structural(StructuralKind::Decision),
        "data-type" => AtomKindClass::Structural(StructuralKind::DataType),
        "message-definition" => AtomKindClass::Structural(StructuralKind::MessageDefinition),
        "timer-definition" => AtomKindClass::Structural(StructuralKind::TimerDefinition),
        "error-definition" => AtomKindClass::Structural(StructuralKind::ErrorDefinition),
        "graph-pack" => AtomKindClass::Structural(StructuralKind::GraphPack),
        "utterance-binding" => AtomKindClass::Structural(StructuralKind::UtteranceBinding),
        "constellation-root" => AtomKindClass::Structural(StructuralKind::ConstellationRoot),
        "workspace-constraint" => AtomKindClass::Structural(StructuralKind::WorkspaceConstraint),
        "decision-pack" => AtomKindClass::Structural(StructuralKind::DecisionPack),

        // Declarative kinds
        "provenance" => AtomKindClass::Declarative(DeclarativeKind::Provenance),
        "governance-status" => AtomKindClass::Declarative(DeclarativeKind::GovernanceStatus),
        "review-annotation" => AtomKindClass::Declarative(DeclarativeKind::ReviewAnnotation),
        "jurisdiction-tag" => AtomKindClass::Declarative(DeclarativeKind::JurisdictionTag),

        // All unknown kinds are structural parse errors in v0.1
        other => AtomKindClass::UnknownStructural(other.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_structural_kinds() {
        assert_eq!(classify("verb"), AtomKindClass::Structural(StructuralKind::Verb));
        assert_eq!(classify("gateway"), AtomKindClass::Structural(StructuralKind::Gateway));
        assert_eq!(classify("boundary-attachment"), AtomKindClass::Structural(StructuralKind::BoundaryAttachment));
        assert_eq!(classify("decision-pack"), AtomKindClass::Structural(StructuralKind::DecisionPack));
    }

    #[test]
    fn classify_declarative_kinds() {
        assert_eq!(classify("provenance"), AtomKindClass::Declarative(DeclarativeKind::Provenance));
        assert_eq!(classify("governance-status"), AtomKindClass::Declarative(DeclarativeKind::GovernanceStatus));
        assert_eq!(classify("jurisdiction-tag"), AtomKindClass::Declarative(DeclarativeKind::JurisdictionTag));
    }

    #[test]
    fn classify_unknown_is_unknown_structural() {
        assert_eq!(
            classify("not-a-real-kind"),
            AtomKindClass::UnknownStructural("not-a-real-kind".to_owned())
        );
    }
}
