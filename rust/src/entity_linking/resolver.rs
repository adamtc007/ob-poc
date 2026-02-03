//! Entity linking service - resolves entity mentions to canonical IDs
//!
//! The `EntityLinkingService` provides:
//! - Multi-mention extraction from utterances
//! - Disambiguation using kind constraints and concept overlap
//! - Stable, serializable evidence for audit trails

use super::mention::{MentionExtractor, MentionSpan};
use super::normalize::normalize_entity_text;
use super::snapshot::{EntityId, EntityRow, EntitySnapshot};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// Evidence for scoring decisions - MUST be Serialize for audit stability
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Evidence {
    /// Exact alias match (score 1.0)
    #[serde(rename = "alias_exact")]
    AliasExact { alias: String },

    /// Token overlap match (score < 1.0)
    #[serde(rename = "alias_token_overlap")]
    AliasTokenOverlap { tokens: Vec<String>, overlap: f32 },

    /// Kind matches expected constraint (boost)
    #[serde(rename = "kind_match_boost")]
    KindMatchBoost {
        expected: String,
        actual: String,
        boost: f32,
    },

    /// Kind doesn't match expected constraint (penalty)
    #[serde(rename = "kind_mismatch_penalty")]
    KindMismatchPenalty {
        expected: String,
        actual: String,
        penalty: f32,
    },

    /// Concept overlap with context (boost)
    #[serde(rename = "concept_overlap_boost")]
    ConceptOverlapBoost { concepts: Vec<String>, boost: f32 },
}

/// A candidate entity match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCandidate {
    /// Entity UUID
    pub entity_id: EntityId,
    /// Entity type/kind
    pub entity_kind: String,
    /// Canonical display name
    pub canonical_name: String,
    /// Final score after adjustments
    pub score: f32,
    /// Evidence chain explaining the score
    pub evidence: Vec<Evidence>,
}

/// Resolution result for a single mention span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityResolution {
    /// Character span in original utterance (start, end)
    pub mention_span: (usize, usize),
    /// Original text of the mention
    pub mention_text: String,
    /// All candidate matches (sorted by score descending)
    pub candidates: Vec<EntityCandidate>,
    /// Selected winner (if unambiguous)
    pub selected: Option<EntityId>,
    /// Confidence in selection (0.0-1.0)
    pub confidence: f32,
    /// Evidence for selection decision
    pub evidence: Vec<Evidence>,
}

/// Trait for entity linking operations
pub trait EntityLinkingService: Send + Sync {
    /// Get snapshot content hash for cache invalidation
    fn snapshot_hash(&self) -> &str;

    /// Get snapshot version
    fn snapshot_version(&self) -> u32;

    /// Resolve entity mentions from utterance.
    /// Returns multiple EntityResolution entries (one per mention span).
    fn resolve_mentions(
        &self,
        utterance: &str,
        expected_kinds: Option<&[String]>,
        context_concepts: Option<&[String]>,
        limit: usize,
    ) -> Vec<EntityResolution>;

    /// Direct lookup by name
    fn lookup_by_name(&self, name: &str, limit: usize) -> Vec<EntityCandidate>;

    /// Direct lookup by ID
    fn lookup_by_id(&self, entity_id: &EntityId) -> Option<EntityRow>;
}

/// Default implementation of EntityLinkingService
pub struct EntityLinkingServiceImpl {
    snapshot: Arc<EntitySnapshot>,
    extractor: MentionExtractor,
}

impl EntityLinkingServiceImpl {
    /// Create with pre-loaded snapshot
    pub fn new(snapshot: Arc<EntitySnapshot>) -> Self {
        Self {
            snapshot,
            extractor: MentionExtractor::default(),
        }
    }

    /// Load from default snapshot path
    pub fn load_default() -> anyhow::Result<Self> {
        let path = Path::new("rust/assets/entity.snapshot.bin");
        Ok(Self::new(Arc::new(EntitySnapshot::load(path)?)))
    }

    /// Load from specific path
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        Ok(Self::new(Arc::new(EntitySnapshot::load(path)?)))
    }

    /// Get reference to underlying snapshot
    pub fn snapshot(&self) -> &EntitySnapshot {
        &self.snapshot
    }
}

// Scoring constants
const KIND_MATCH_BOOST: f32 = 0.05;
const KIND_MISMATCH_PENALTY: f32 = 0.20;
const MAX_CONCEPT_BOOST: f32 = 0.10;
const SELECTION_THRESHOLD: f32 = 0.50;
const AMBIGUITY_MARGIN: f32 = 0.08;

impl EntityLinkingService for EntityLinkingServiceImpl {
    fn snapshot_hash(&self) -> &str {
        &self.snapshot.hash
    }

    fn snapshot_version(&self) -> u32 {
        self.snapshot.version
    }

    fn resolve_mentions(
        &self,
        utterance: &str,
        expected_kinds: Option<&[String]>,
        context_concepts: Option<&[String]>,
        limit: usize,
    ) -> Vec<EntityResolution> {
        // Extract mention spans
        let spans = self.extractor.extract(utterance, &self.snapshot);

        if spans.is_empty() {
            return vec![];
        }

        spans
            .into_iter()
            .map(|span| self.resolve_span(span, expected_kinds, context_concepts, limit))
            .collect()
    }

    fn lookup_by_name(&self, name: &str, limit: usize) -> Vec<EntityCandidate> {
        let normalized = normalize_entity_text(name, false);
        let spans = self.extractor.extract(&normalized, &self.snapshot);

        spans
            .into_iter()
            .flat_map(|s| s.candidate_ids.into_iter())
            .filter_map(|id| {
                let row = self.snapshot.get(&id)?;
                Some(EntityCandidate {
                    entity_id: id,
                    entity_kind: row.entity_kind.clone(),
                    canonical_name: row.canonical_name.clone(),
                    score: 1.0,
                    evidence: vec![],
                })
            })
            .take(limit)
            .collect()
    }

    fn lookup_by_id(&self, entity_id: &EntityId) -> Option<EntityRow> {
        self.snapshot.get(entity_id).cloned()
    }
}

impl EntityLinkingServiceImpl {
    /// Resolve a single mention span to candidates
    fn resolve_span(
        &self,
        span: MentionSpan,
        expected_kinds: Option<&[String]>,
        context_concepts: Option<&[String]>,
        limit: usize,
    ) -> EntityResolution {
        let mut candidates: Vec<EntityCandidate> = span
            .candidate_ids
            .iter()
            .filter_map(|id| {
                let row = self.snapshot.get(id)?;

                // Build initial evidence
                let evidence = if span.score >= 1.0 {
                    vec![Evidence::AliasExact {
                        alias: span.normalized.clone(),
                    }]
                } else {
                    vec![Evidence::AliasTokenOverlap {
                        tokens: span.tokens.clone(),
                        overlap: span.score,
                    }]
                };

                Some(EntityCandidate {
                    entity_id: *id,
                    entity_kind: row.entity_kind.clone(),
                    canonical_name: row.canonical_name.clone(),
                    score: span.score,
                    evidence,
                })
            })
            .collect();

        // Apply scoring adjustments
        for c in &mut candidates {
            self.apply_scoring(c, expected_kinds, context_concepts);
        }

        // Sort by score descending
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(limit);

        // Select winner
        let (selected, confidence, evidence) = self.select_winner(&candidates);

        EntityResolution {
            mention_span: (span.start, span.end),
            mention_text: span.text,
            candidates,
            selected,
            confidence,
            evidence,
        }
    }

    /// Apply scoring adjustments for kind constraints and concept overlap
    fn apply_scoring(
        &self,
        candidate: &mut EntityCandidate,
        expected_kinds: Option<&[String]>,
        context_concepts: Option<&[String]>,
    ) {
        // Kind constraint
        if let Some(kinds) = expected_kinds {
            if !kinds.is_empty() {
                let kind_lower = candidate.entity_kind.to_lowercase();
                let matches = kinds.iter().any(|k| k.to_lowercase() == kind_lower);

                if matches {
                    candidate.score = (candidate.score + KIND_MATCH_BOOST).min(1.0);
                    candidate.evidence.push(Evidence::KindMatchBoost {
                        expected: kinds.join("|"),
                        actual: candidate.entity_kind.clone(),
                        boost: KIND_MATCH_BOOST,
                    });
                } else {
                    candidate.score = (candidate.score - KIND_MISMATCH_PENALTY).max(0.0);
                    candidate.evidence.push(Evidence::KindMismatchPenalty {
                        expected: kinds.join("|"),
                        actual: candidate.entity_kind.clone(),
                        penalty: KIND_MISMATCH_PENALTY,
                    });
                }
            }
        }

        // Concept overlap
        if let Some(ctx) = context_concepts {
            if let Some(links) = self.snapshot.get_concepts(&candidate.entity_id) {
                let overlap: Vec<String> = links
                    .iter()
                    .filter(|(cid, _)| ctx.contains(cid))
                    .map(|(cid, _)| cid.clone())
                    .collect();

                if !overlap.is_empty() {
                    let boost = (overlap.len() as f32 * 0.03).min(MAX_CONCEPT_BOOST);
                    candidate.score = (candidate.score + boost).min(1.0);
                    candidate.evidence.push(Evidence::ConceptOverlapBoost {
                        concepts: overlap,
                        boost,
                    });
                }
            }
        }
    }

    /// Select winner from candidates using threshold and margin
    fn select_winner(
        &self,
        candidates: &[EntityCandidate],
    ) -> (Option<EntityId>, f32, Vec<Evidence>) {
        match candidates.len() {
            0 => (None, 0.0, vec![]),
            1 => {
                let c = &candidates[0];
                if c.score >= SELECTION_THRESHOLD {
                    (Some(c.entity_id), c.score, c.evidence.clone())
                } else {
                    (None, c.score, vec![])
                }
            }
            _ => {
                let top = &candidates[0];
                let second = &candidates[1];
                if top.score >= SELECTION_THRESHOLD && (top.score - second.score) > AMBIGUITY_MARGIN
                {
                    (Some(top.entity_id), top.score, top.evidence.clone())
                } else {
                    // Ambiguous - no clear winner
                    (None, top.score, vec![])
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_serializable() {
        let evidence = Evidence::AliasTokenOverlap {
            tokens: vec!["stellar".to_string(), "corp".to_string()],
            overlap: 0.67,
        };

        let json = serde_json::to_string(&evidence).unwrap();
        let parsed: Evidence = serde_json::from_str(&json).unwrap();

        assert_eq!(evidence, parsed);
    }

    #[test]
    fn test_evidence_tagged_serialization() {
        let evidence = Evidence::AliasExact {
            alias: "apple".to_string(),
        };

        let json = serde_json::to_string(&evidence).unwrap();
        assert!(json.contains(r#""type":"alias_exact""#));
    }

    #[test]
    fn test_kind_match_evidence() {
        let evidence = Evidence::KindMatchBoost {
            expected: "company".to_string(),
            actual: "company".to_string(),
            boost: 0.05,
        };

        let json = serde_json::to_string(&evidence).unwrap();
        assert!(json.contains(r#""type":"kind_match_boost""#));
    }
}
