//! LexiconService - Runtime interface for lexicon queries.
//!
//! The service wraps a LexiconSnapshot and provides high-level query methods
//! for verb search, entity type search, and domain inference.
//!
//! ## Performance Requirements
//!
//! - All methods must be in-memory only (no DB, no file I/O)
//! - Target: <100µs per call
//! - Bounded iterations (max 8 concepts per label, max 16 per token)

use std::collections::HashMap;
use std::sync::Arc;

use super::snapshot::LexiconSnapshot;
use super::types::*;

// =============================================================================
// Trait Definition
// =============================================================================

/// Trait for lexicon service implementations.
///
/// This trait allows for mocking in tests and potential future implementations
/// (e.g., hot-reloading wrapper).
pub trait LexiconService: Send + Sync {
    /// Get the hash of the underlying snapshot (for cache invalidation).
    fn snapshot_hash(&self) -> &str;

    /// Search for verbs matching a phrase.
    ///
    /// - `phrase`: The user's input (will be normalized)
    /// - `target_type`: Optional entity type context for scoring
    /// - `limit`: Maximum number of candidates to return
    fn search_verbs(
        &self,
        phrase: &str,
        target_type: Option<&str>,
        limit: usize,
    ) -> Vec<VerbCandidate>;

    /// Search for entity types matching a phrase.
    fn search_entity_types(&self, phrase: &str, limit: usize) -> Vec<EntityTypeCandidate>;

    /// Get target types for a verb (for constraint checking).
    fn verb_target_types(&self, dsl_verb: &str) -> Vec<String>;

    /// Get the domain for a verb.
    fn verb_domain(&self, dsl_verb: &str) -> Option<String>;

    /// Get the type produced by a verb (for chaining).
    fn verb_produces_type(&self, dsl_verb: &str) -> Option<String>;

    /// Infer domain from a phrase using keyword matching.
    fn infer_domain(&self, phrase: &str) -> Option<String>;
}

// =============================================================================
// Implementation
// =============================================================================

/// Standard implementation of LexiconService backed by a snapshot.
pub struct LexiconServiceImpl {
    snapshot: Arc<LexiconSnapshot>,
}

impl LexiconServiceImpl {
    /// Create a new service wrapping a snapshot.
    pub fn new(snapshot: Arc<LexiconSnapshot>) -> Self {
        Self { snapshot }
    }

    /// Normalize a phrase for lookup: lowercase, collapse whitespace.
    #[inline]
    fn normalize(phrase: &str) -> String {
        phrase
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Extract verb name from concept ID (e.g., "verb.cbu.create" → "cbu.create").
    #[inline]
    fn extract_verb_from_concept(concept_id: &str) -> Option<&str> {
        concept_id.strip_prefix("verb.")
    }

    /// Extract entity type from concept ID (e.g., "entity_type.fund" → "fund").
    #[inline]
    fn extract_entity_type_from_concept(concept_id: &str) -> Option<&str> {
        concept_id.strip_prefix("entity_type.")
    }

    /// Compute target type match for a verb.
    fn compute_target_match(&self, meta: &VerbMeta, target_type: Option<&str>) -> TargetTypeMatch {
        match target_type {
            None => TargetTypeMatch::NoTarget,
            Some(_) if meta.target_types.is_empty() => TargetTypeMatch::NoConstraint,
            Some(tt) if meta.target_types.iter().any(|x| x == tt) => TargetTypeMatch::Matched {
                matched_type: tt.to_string(),
            },
            Some(tt) => TargetTypeMatch::Mismatched {
                expected: meta.target_types.clone(),
                got: tt.to_string(),
            },
        }
    }
}

impl LexiconService for LexiconServiceImpl {
    fn snapshot_hash(&self) -> &str {
        &self.snapshot.hash
    }

    fn search_verbs(
        &self,
        phrase: &str,
        target_type: Option<&str>,
        limit: usize,
    ) -> Vec<VerbCandidate> {
        let norm = Self::normalize(phrase);
        let mut candidates: Vec<VerbCandidate> = Vec::new();
        // Use owned Strings to avoid lifetime issues between pass 1 and pass 2
        let mut seen_verbs: std::collections::HashSet<String> = std::collections::HashSet::new();

        // =====================================================================
        // Pass 1: Exact label match (highest confidence)
        // =====================================================================
        if let Some(concepts) = self.snapshot.get_concepts_for_label(&norm) {
            // Bounded: max 8 concepts per label
            for concept_id in concepts.iter().take(8) {
                if let Some(dsl_verb) = Self::extract_verb_from_concept(concept_id) {
                    if seen_verbs.contains(dsl_verb) {
                        continue;
                    }

                    if let Some(meta) = self.snapshot.get_verb_meta(dsl_verb) {
                        let target_match = self.compute_target_match(meta, target_type);
                        let mut score = 1.0_f32;
                        score = (score + target_match.score_adjustment()).clamp(0.0, 1.0);

                        let dsl_verb_owned = dsl_verb.to_string();
                        candidates.push(VerbCandidate {
                            dsl_verb: dsl_verb_owned.clone(),
                            score,
                            evidence: smallvec::smallvec![MatchEvidence::PrefLabel {
                                label: norm.clone(),
                                score,
                            }],
                            target_type_match: target_match,
                        });

                        seen_verbs.insert(dsl_verb_owned);
                    }
                }
            }
        }

        // =====================================================================
        // Pass 2: Token overlap match (partial match)
        // =====================================================================
        let tokens: Vec<&str> = norm.split_whitespace().take(8).collect();
        if tokens.is_empty() {
            candidates.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            candidates.truncate(limit);
            return candidates;
        }

        // Accumulate token overlap scores per verb
        let mut token_scores: HashMap<String, (f32, Vec<String>)> = HashMap::new();
        let token_weight = 1.0 / tokens.len() as f32;

        for token in tokens.iter() {
            if let Some(concepts) = self.snapshot.get_concepts_for_token(token) {
                // Bounded: max 16 concepts per token
                for concept_id in concepts.iter().take(16) {
                    if let Some(dsl_verb) = Self::extract_verb_from_concept(concept_id) {
                        // Skip if already found via exact match
                        if seen_verbs.contains(dsl_verb) {
                            continue;
                        }

                        let entry = token_scores
                            .entry(dsl_verb.to_string())
                            .or_insert((0.0, Vec::new()));
                        entry.0 += token_weight;
                        entry.1.push((*token).to_string());
                    }
                }
            }
        }

        // Convert token scores to candidates (threshold: 34% token overlap)
        const TOKEN_OVERLAP_THRESHOLD: f32 = 0.34;
        const TOKEN_OVERLAP_SCALE: f32 = 0.80; // Scale down vs exact match

        for (dsl_verb, (raw_score, matched_tokens)) in token_scores {
            if raw_score < TOKEN_OVERLAP_THRESHOLD {
                continue;
            }

            if let Some(meta) = self.snapshot.get_verb_meta(&dsl_verb) {
                let target_match = self.compute_target_match(meta, target_type);
                let mut score = (raw_score * TOKEN_OVERLAP_SCALE).clamp(0.0, 1.0);
                score = (score + target_match.score_adjustment()).clamp(0.0, 1.0);

                candidates.push(VerbCandidate {
                    dsl_verb,
                    score,
                    evidence: smallvec::smallvec![MatchEvidence::TokenOverlap {
                        matched_tokens,
                        score,
                    }],
                    target_type_match: target_match,
                });
            }
        }

        // Sort by score descending, truncate to limit
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(limit);

        candidates
    }

    fn search_entity_types(&self, phrase: &str, limit: usize) -> Vec<EntityTypeCandidate> {
        let norm = Self::normalize(phrase);
        let mut candidates: Vec<EntityTypeCandidate> = Vec::new();

        if let Some(concepts) = self.snapshot.get_concepts_for_label(&norm) {
            for concept_id in concepts.iter().take(limit) {
                if let Some(type_name) = Self::extract_entity_type_from_concept(concept_id) {
                    if let Some(meta) = self.snapshot.get_entity_type_meta(type_name) {
                        candidates.push(EntityTypeCandidate {
                            type_name: type_name.to_string(),
                            score: 1.0,
                            matched_alias: norm.clone(),
                            domain: meta.domain.clone(),
                        });
                    }
                }
            }
        }

        candidates
    }

    fn verb_target_types(&self, dsl_verb: &str) -> Vec<String> {
        self.snapshot
            .get_verb_meta(dsl_verb)
            .map(|m| m.target_types.clone())
            .unwrap_or_default()
    }

    fn verb_domain(&self, dsl_verb: &str) -> Option<String> {
        self.snapshot
            .get_verb_meta(dsl_verb)
            .and_then(|m| m.domain.clone())
    }

    fn verb_produces_type(&self, dsl_verb: &str) -> Option<String> {
        self.snapshot
            .get_verb_meta(dsl_verb)
            .and_then(|m| m.produces_type.clone())
    }

    fn infer_domain(&self, phrase: &str) -> Option<String> {
        // Bounded: check first 16 words only
        for word in phrase.to_lowercase().split_whitespace().take(16) {
            if let Some(domain) = self.snapshot.get_domain_for_keyword(word) {
                return Some(domain.clone());
            }
        }
        None
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    fn make_test_snapshot() -> Arc<LexiconSnapshot> {
        let mut snapshot = LexiconSnapshot::empty();
        snapshot.hash = "test_snapshot".to_string();

        // Add verb metadata
        snapshot.verb_meta.insert(
            "cbu.create".to_string(),
            VerbMeta {
                dsl_verb: "cbu.create".to_string(),
                pref_label: "Create CBU".to_string(),
                domain: Some("cbu".to_string()),
                target_types: vec!["fund".to_string(), "mandate".to_string()],
                crud_type: Some("create".to_string()),
                alt_labels: vec!["spin up".to_string(), "new fund".to_string()],
                ..Default::default()
            },
        );

        snapshot.verb_meta.insert(
            "cbu.list".to_string(),
            VerbMeta {
                dsl_verb: "cbu.list".to_string(),
                pref_label: "List CBUs".to_string(),
                domain: Some("cbu".to_string()),
                crud_type: Some("read".to_string()),
                ..Default::default()
            },
        );

        // Add label→concept index
        snapshot.label_to_concepts.insert(
            "create cbu".to_string(),
            smallvec!["verb.cbu.create".to_string()],
        );
        snapshot.label_to_concepts.insert(
            "spin up".to_string(),
            smallvec!["verb.cbu.create".to_string()],
        );
        snapshot.label_to_concepts.insert(
            "list cbus".to_string(),
            smallvec!["verb.cbu.list".to_string()],
        );

        // Add token→concept index
        snapshot.token_to_concepts.insert(
            "create".to_string(),
            smallvec!["verb.cbu.create".to_string()],
        );
        snapshot.token_to_concepts.insert(
            "cbu".to_string(),
            smallvec!["verb.cbu.create".to_string(), "verb.cbu.list".to_string()],
        );
        snapshot
            .token_to_concepts
            .insert("list".to_string(), smallvec!["verb.cbu.list".to_string()]);

        // Add domain keywords
        snapshot
            .keyword_to_domain
            .insert("cbu".to_string(), "cbu".to_string());
        snapshot
            .keyword_to_domain
            .insert("fund".to_string(), "cbu".to_string());
        snapshot
            .keyword_to_domain
            .insert("kyc".to_string(), "kyc".to_string());

        Arc::new(snapshot)
    }

    #[test]
    fn test_search_verbs_exact_match() {
        let snapshot = make_test_snapshot();
        let service = LexiconServiceImpl::new(snapshot);

        let results = service.search_verbs("create cbu", None, 5);
        // Should find cbu.create via exact match (score 1.0) AND cbu.list via token overlap
        // (token "cbu" maps to both verbs). The exact match should be first (higher score).
        assert!(!results.is_empty(), "Should find at least exact match");
        assert_eq!(
            results[0].dsl_verb, "cbu.create",
            "Exact match should be first"
        );
        assert!(
            (results[0].score - 1.0).abs() < 0.001,
            "Exact match should score 1.0"
        );
        // Verify it's from exact label match
        assert!(
            matches!(
                results[0].evidence.first(),
                Some(MatchEvidence::PrefLabel { .. })
            ),
            "Should be PrefLabel evidence"
        );
    }

    #[test]
    fn test_search_verbs_with_target_type_match() {
        let snapshot = make_test_snapshot();
        let service = LexiconServiceImpl::new(snapshot);

        let results = service.search_verbs("create cbu", Some("fund"), 5);
        // May return multiple results (exact + token overlap); check top result
        assert!(!results.is_empty(), "Should find at least one result");
        assert_eq!(results[0].dsl_verb, "cbu.create");
        // Score should be 1.0 + 0.05 bonus, clamped to 1.0
        assert!((results[0].score - 1.0).abs() < 0.001);
        assert!(matches!(
            results[0].target_type_match,
            TargetTypeMatch::Matched { .. }
        ));
    }

    #[test]
    fn test_search_verbs_with_target_type_mismatch() {
        let snapshot = make_test_snapshot();
        let service = LexiconServiceImpl::new(snapshot);

        let results = service.search_verbs("create cbu", Some("person"), 5);
        // May return multiple results; check top result
        assert!(!results.is_empty(), "Should find at least one result");
        // Score should be 1.0 - 0.10 penalty = 0.90
        assert!((results[0].score - 0.90).abs() < 0.001);
        assert!(matches!(
            results[0].target_type_match,
            TargetTypeMatch::Mismatched { .. }
        ));
    }

    #[test]
    fn test_search_verbs_token_overlap() {
        let snapshot = make_test_snapshot();
        let service = LexiconServiceImpl::new(snapshot);

        // "cbu" token matches both cbu.create and cbu.list
        let results = service.search_verbs("cbu", None, 5);
        // Should find candidates via token overlap
        assert!(!results.is_empty());
    }

    #[test]
    fn test_infer_domain() {
        let snapshot = make_test_snapshot();
        let service = LexiconServiceImpl::new(snapshot);

        assert_eq!(
            service.infer_domain("create a new fund"),
            Some("cbu".to_string())
        );
        assert_eq!(
            service.infer_domain("kyc case review"),
            Some("kyc".to_string())
        );
        assert_eq!(service.infer_domain("random text"), None);
    }

    #[test]
    fn test_verb_metadata_lookup() {
        let snapshot = make_test_snapshot();
        let service = LexiconServiceImpl::new(snapshot);

        let targets = service.verb_target_types("cbu.create");
        assert!(targets.contains(&"fund".to_string()));

        let domain = service.verb_domain("cbu.create");
        assert_eq!(domain, Some("cbu".to_string()));

        // Unknown verb
        let unknown_domain = service.verb_domain("unknown.verb");
        assert!(unknown_domain.is_none());
    }

    #[test]
    fn test_normalize() {
        assert_eq!(
            LexiconServiceImpl::normalize("  Create  CBU  "),
            "create cbu"
        );
        assert_eq!(LexiconServiceImpl::normalize("SPIN UP"), "spin up");
    }
}
