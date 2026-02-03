//! Lexicon types for in-memory vocabulary lookup.
//!
//! These types are separate from verb_search types to avoid coupling.
//! Conversion to VerbSearchResult happens in the integration layer.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Concept identifier (e.g., "verb.cbu.create", "entity_type.fund")
pub type ConceptId = String;

/// Normalized label for lookup (lowercase, whitespace-collapsed)
pub type LabelNorm = String;

// =============================================================================
// Match Evidence
// =============================================================================

/// Evidence for why a concept matched a query.
///
/// This is lexicon-internal evidence. When returning VerbSearchResult,
/// convert to VerbEvidence with appropriate VerbSearchSource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchEvidence {
    /// Matched the preferred label exactly
    PrefLabel { label: String, score: f32 },
    /// Matched an alternate label (synonym)
    AltLabel { label: String, score: f32 },
    /// Matched an invocation phrase
    InvocationPhrase { phrase: String, score: f32 },
    /// Token overlap match (partial)
    TokenOverlap {
        matched_tokens: Vec<String>,
        score: f32,
    },
}

impl MatchEvidence {
    /// Get the score from this evidence
    pub fn score(&self) -> f32 {
        match self {
            Self::PrefLabel { score, .. } => *score,
            Self::AltLabel { score, .. } => *score,
            Self::InvocationPhrase { score, .. } => *score,
            Self::TokenOverlap { score, .. } => *score,
        }
    }

    /// Get the matched phrase/label from this evidence
    pub fn matched_text(&self) -> &str {
        match self {
            Self::PrefLabel { label, .. } => label,
            Self::AltLabel { label, .. } => label,
            Self::InvocationPhrase { phrase, .. } => phrase,
            Self::TokenOverlap { matched_tokens, .. } => {
                // Return first token or empty
                matched_tokens.first().map(|s| s.as_str()).unwrap_or("")
            }
        }
    }
}

// =============================================================================
// Target Type Matching
// =============================================================================

/// Result of checking if a verb's target types match the current context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TargetTypeMatch {
    /// Verb has no target type constraints
    NoConstraint,
    /// Target type matches one of the verb's expected types
    Matched { matched_type: String },
    /// Target type does NOT match any expected types
    Mismatched { expected: Vec<String>, got: String },
    /// No target type was provided for matching
    NoTarget,
}

impl TargetTypeMatch {
    /// Returns true if this is a successful match or no constraint
    pub fn is_acceptable(&self) -> bool {
        matches!(
            self,
            Self::NoConstraint | Self::Matched { .. } | Self::NoTarget
        )
    }

    /// Get score adjustment for target type match
    /// Small bonus for match, small penalty for mismatch, clamped to [0,1]
    pub fn score_adjustment(&self) -> f32 {
        match self {
            Self::Matched { .. } => 0.05,
            Self::Mismatched { .. } => -0.10,
            Self::NoConstraint | Self::NoTarget => 0.0,
        }
    }
}

// =============================================================================
// Candidates
// =============================================================================

/// A verb candidate from lexicon search.
///
/// This is lexicon-internal. Convert to VerbSearchResult for pipeline use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCandidate {
    /// The DSL verb name (e.g., "cbu.create")
    pub dsl_verb: String,
    /// Match score, clamped to [0, 1]
    pub score: f32,
    /// Evidence for why this verb matched
    pub evidence: SmallVec<[MatchEvidence; 4]>,
    /// Target type match result
    pub target_type_match: TargetTypeMatch,
}

impl VerbCandidate {
    /// Create a new verb candidate with a single evidence item
    pub fn new(dsl_verb: String, score: f32, evidence: MatchEvidence) -> Self {
        Self {
            dsl_verb,
            score: score.clamp(0.0, 1.0),
            evidence: smallvec::smallvec![evidence],
            target_type_match: TargetTypeMatch::NoTarget,
        }
    }

    /// Add target type match and adjust score
    pub fn with_target_type(mut self, target_match: TargetTypeMatch) -> Self {
        self.score = (self.score + target_match.score_adjustment()).clamp(0.0, 1.0);
        self.target_type_match = target_match;
        self
    }
}

/// An entity type candidate from lexicon search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTypeCandidate {
    /// The entity type name (e.g., "fund", "legal_entity")
    pub type_name: String,
    /// Match score, clamped to [0, 1]
    pub score: f32,
    /// The alias that matched
    pub matched_alias: String,
    /// Domain this entity type belongs to
    pub domain: Option<String>,
}

// =============================================================================
// Metadata
// =============================================================================

/// Metadata about a verb stored in the lexicon.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VerbMeta {
    /// The DSL verb name (e.g., "cbu.create")
    pub dsl_verb: String,
    /// Preferred label for display
    pub pref_label: String,
    /// Domain this verb belongs to (e.g., "cbu", "session")
    pub domain: Option<String>,
    /// Entity types this verb can operate on
    pub target_types: Vec<String>,
    /// Entity type this verb produces (for chaining)
    pub produces_type: Option<String>,
    /// CRUD type (create, read, update, delete, link, navigate)
    pub crud_type: Option<String>,
    /// Invocation phrases for this verb
    pub invocation_phrases: Vec<String>,
    /// Alternate labels (synonyms)
    pub alt_labels: Vec<String>,
}

/// Metadata about an entity type stored in the lexicon.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntityTypeMeta {
    /// The entity type name
    pub type_name: String,
    /// Preferred label for display
    pub pref_label: String,
    /// Aliases for this entity type
    pub aliases: Vec<String>,
    /// Domain this entity type belongs to
    pub domain: Option<String>,
}

/// Metadata about a domain.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomainMeta {
    /// Domain identifier (e.g., "cbu", "session", "kyc")
    pub domain_id: String,
    /// Display label
    pub label: String,
    /// Parent domain (for hierarchy)
    pub parent: Option<String>,
    /// Keywords that infer this domain
    pub inference_keywords: Vec<String>,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verb_candidate_score_clamping() {
        let candidate = VerbCandidate::new(
            "cbu.create".to_string(),
            1.5, // Over 1.0
            MatchEvidence::PrefLabel {
                label: "create".to_string(),
                score: 1.5,
            },
        );
        assert!(candidate.score <= 1.0, "Score should be clamped to 1.0");

        let candidate2 = VerbCandidate::new(
            "cbu.create".to_string(),
            -0.5, // Under 0.0
            MatchEvidence::PrefLabel {
                label: "create".to_string(),
                score: -0.5,
            },
        );
        assert!(candidate2.score >= 0.0, "Score should be clamped to 0.0");
    }

    #[test]
    fn test_target_type_match_adjustment() {
        let candidate = VerbCandidate::new(
            "cbu.create".to_string(),
            0.9,
            MatchEvidence::PrefLabel {
                label: "create".to_string(),
                score: 0.9,
            },
        )
        .with_target_type(TargetTypeMatch::Matched {
            matched_type: "fund".to_string(),
        });

        assert!(
            (candidate.score - 0.95).abs() < 0.001,
            "Score should be 0.95 after +0.05 bonus"
        );

        let candidate2 = VerbCandidate::new(
            "cbu.create".to_string(),
            0.9,
            MatchEvidence::PrefLabel {
                label: "create".to_string(),
                score: 0.9,
            },
        )
        .with_target_type(TargetTypeMatch::Mismatched {
            expected: vec!["entity".to_string()],
            got: "fund".to_string(),
        });

        assert!(
            (candidate2.score - 0.8).abs() < 0.001,
            "Score should be 0.8 after -0.10 penalty"
        );
    }

    #[test]
    fn test_match_evidence_score() {
        let evidence = MatchEvidence::AltLabel {
            label: "spin up".to_string(),
            score: 0.85,
        };
        assert!((evidence.score() - 0.85).abs() < 0.001);
    }
}
