//! Unified LookupService - Verb-first dual search
//!
//! Combines verb discovery and entity resolution in a single pass,
//! using verb schema to constrain entity kinds.

use crate::entity_linking::{EntityLinkingService, EntityResolution};
use crate::lexicon::LexiconService;
use crate::mcp::verb_search::{HybridVerbSearcher, VerbSearchResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Result of unified lookup analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupResult {
    /// Verb candidates from search (sorted by score)
    pub verbs: Vec<VerbSearchResult>,

    /// Entity resolutions with kind-constrained scoring
    pub entities: Vec<EntityResolution>,

    /// Dominant entity (highest confidence, kind-matched)
    pub dominant_entity: Option<DominantEntity>,

    /// Expected entity kinds derived from top verb(s)
    pub expected_kinds: Vec<String>,

    /// Concepts extracted from utterance (for context)
    pub concepts: Vec<String>,

    /// Whether verb search found a clear winner
    pub verb_matched: bool,

    /// Whether entity resolution found unambiguous matches
    pub entities_resolved: bool,
}

/// The dominant entity from analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DominantEntity {
    pub entity_id: Uuid,
    pub canonical_name: String,
    pub entity_kind: String,
    pub confidence: f32,
    pub mention_span: (usize, usize),
}

/// Unified lookup service combining verb search and entity linking
pub struct LookupService {
    /// Entity linking for mention extraction and resolution
    entity_linker: Arc<dyn EntityLinkingService>,

    /// Lexicon for concept extraction (optional, for future use)
    lexicon: Option<Arc<dyn LexiconService>>,

    /// Verb searcher for semantic verb discovery
    verb_searcher: Option<Arc<HybridVerbSearcher>>,
}

impl LookupService {
    /// Create with entity linker only (verb searcher set separately)
    pub fn new(entity_linker: Arc<dyn EntityLinkingService>) -> Self {
        Self {
            entity_linker,
            lexicon: None,
            verb_searcher: None,
        }
    }

    /// Set verb searcher
    pub fn with_verb_searcher(mut self, searcher: Arc<HybridVerbSearcher>) -> Self {
        self.verb_searcher = Some(searcher);
        self
    }

    /// Set lexicon service
    pub fn with_lexicon(mut self, lexicon: Arc<dyn LexiconService>) -> Self {
        self.lexicon = Some(lexicon);
        self
    }

    /// Analyze utterance with entity-masked verb search
    ///
    /// 1. Extract entity mention spans (fast, in-memory)
    /// 2. Search verbs with entity names masked from ECIR noun scan
    /// 3. Derive expected entity kinds from verb schema
    /// 4. Resolve entities with kind constraints
    pub async fn analyze(&self, utterance: &str, limit: usize) -> LookupResult {
        // Step 0: Extract entity mention spans before verb search.
        // These spans tell the ECIR noun scanner to skip entity names
        // (e.g., "Goldman Sachs Group") so they don't pollute domain noun matching.
        let mention_spans = self.entity_linker.extract_mention_spans(utterance);
        let spans_ref: Option<&[(usize, usize)]> = if mention_spans.is_empty() {
            None
        } else {
            tracing::debug!(
                span_count = mention_spans.len(),
                spans = ?mention_spans,
                "LookupService: masking entity mention spans for ECIR"
            );
            Some(&mention_spans)
        };

        // Step 1: Verb search with entity names masked from ECIR
        let verbs = if let Some(searcher) = &self.verb_searcher {
            searcher
                .search(utterance, None, None, None, limit, None, spans_ref)
                .await
                .unwrap_or_default()
        } else {
            vec![]
        };

        let verb_matched = verbs
            .first()
            .map(|v| v.score >= 0.65) // Clear match threshold
            .unwrap_or(false);

        // Step 2: Derive expected kinds from top verb(s)
        let expected_kinds = self.derive_expected_kinds(&verbs);

        // Step 3: Entity resolution with kind constraints (informed by verb schema)
        let kind_refs: Vec<String> = expected_kinds.clone();
        let kind_constraint = if kind_refs.is_empty() {
            None
        } else {
            Some(kind_refs.as_slice())
        };

        let entities = self.entity_linker.resolve_mentions(
            utterance,
            kind_constraint,
            None, // No concept context for now
            limit,
        );

        // Step 4: Co-resolution — entity kind ↔ verb subject_kinds mutual boosting
        //
        // The verb context narrows entity candidates:
        //   "open a KYC case for HSBC" → kyc-case verbs have subject_kinds=[cbu]
        //   → boost HSBC Custody Services (cbu), penalise HSBC Holdings (group)
        //
        // The entity kind narrows verb candidates:
        //   entity is a fund → boost fund.* verbs, penalise group.* verbs
        let entities = self.co_resolve_entities(&verbs, entities);

        // Step 5: Find dominant entity (after co-resolution re-ranking)
        let dominant_entity = self.find_dominant(&entities);
        let has_dominant = dominant_entity.is_some();

        LookupResult {
            verbs,
            entities,
            dominant_entity,
            expected_kinds,
            concepts: vec![],
            verb_matched,
            entities_resolved: has_dominant,
        }
    }

    /// Derive expected entity kinds from verb schema
    ///
    /// Looks at top verb candidates and extracts entity types
    /// from args that have lookup config.
    fn derive_expected_kinds(&self, verbs: &[VerbSearchResult]) -> Vec<String> {
        use crate::dsl_v2::verb_registry::registry;

        let mut kinds: Vec<String> = Vec::new();
        let reg = registry();

        // Check top 3 verb candidates
        for verb_result in verbs.iter().take(3) {
            let parts: Vec<&str> = verb_result.verb.splitn(2, '.').collect();
            if parts.len() != 2 {
                continue;
            }

            if let Some(verb_def) = reg.get_runtime_verb(parts[0], parts[1]) {
                for arg in &verb_def.args {
                    // Args with lookup config expect entity types
                    if let Some(lookup) = &arg.lookup {
                        if let Some(entity_type) = &lookup.entity_type {
                            if !kinds.contains(entity_type) {
                                kinds.push(entity_type.clone());
                            }
                        }
                    }
                }
            }
        }

        kinds
    }

    /// Co-resolve: boost entity candidates whose kind matches top verb subject_kinds.
    ///
    /// "Open a KYC case for HSBC" → kyc-case.create has subject_kinds=[cbu]
    /// → HSBC Custody Services (cbu) boosted, HSBC Holdings (group) penalised.
    fn co_resolve_entities(
        &self,
        verbs: &[VerbSearchResult],
        mut entities: Vec<EntityResolution>,
    ) -> Vec<EntityResolution> {
        use crate::dsl_v2::runtime_registry::runtime_registry;

        if entities.is_empty() || verbs.is_empty() {
            return entities;
        }

        // Collect subject_kinds from top 3 verb candidates
        let registry = runtime_registry();
        let mut verb_subject_kinds: Vec<String> = Vec::new();
        for verb_result in verbs.iter().take(3) {
            let parts: Vec<&str> = verb_result.verb.splitn(2, '.').collect();
            if parts.len() == 2 {
                if let Some(rv) = registry.get(parts[0], parts[1]) {
                    for kind in &rv.subject_kinds {
                        if !verb_subject_kinds.contains(kind) {
                            verb_subject_kinds.push(kind.clone());
                        }
                    }
                }
            }
        }

        if verb_subject_kinds.is_empty() {
            return entities; // No kind constraint from verbs → no re-ranking
        }

        // Boost/penalise entity candidates based on kind match
        for entity_resolution in &mut entities {
            for candidate in &mut entity_resolution.candidates {
                let kind_matches = verb_subject_kinds.iter().any(|vk| {
                    vk == &candidate.entity_kind
                        || (vk == "entity" && matches!(candidate.entity_kind.as_str(), "person" | "company" | "trust" | "partnership"))
                        || (vk == "cbu" && candidate.entity_kind == "fund")
                });

                if kind_matches {
                    candidate.score += 0.15; // Boost matching kind
                    tracing::debug!(
                        entity = %candidate.canonical_name,
                        kind = %candidate.entity_kind,
                        verb_kinds = ?verb_subject_kinds,
                        "Co-resolution: entity kind matches verb subject_kinds (+0.15)"
                    );
                } else if !verb_subject_kinds.is_empty() {
                    candidate.score -= 0.10; // Penalise mismatching kind
                }
            }

            // Re-sort candidates by score and re-select dominant
            entity_resolution
                .candidates
                .sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

            // Update the selected entity if re-ranking changed the winner
            if let Some(top) = entity_resolution.candidates.first() {
                if top.score > 0.5 {
                    entity_resolution.selected = Some(top.entity_id);
                    entity_resolution.confidence = top.score;
                }
            }
        }

        entities
    }

    /// Find the dominant entity (highest confidence with selection)
    fn find_dominant(&self, entities: &[EntityResolution]) -> Option<DominantEntity> {
        entities
            .iter()
            .filter(|r| r.selected.is_some() && r.confidence > 0.5)
            .max_by(|a, b| {
                a.confidence
                    .partial_cmp(&b.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|r| {
                let candidate = r.candidates.first()?;
                Some(DominantEntity {
                    entity_id: r.selected?,
                    canonical_name: candidate.canonical_name.clone(),
                    entity_kind: candidate.entity_kind.clone(),
                    confidence: r.confidence,
                    mention_span: r.mention_span,
                })
            })
    }
}

#[cfg(test)]
/// Create a stub lookup service for testing
pub fn stub_lookup_service() -> LookupService {
    use crate::entity_linking::StubEntityLinkingService;
    LookupService::new(Arc::new(StubEntityLinkingService))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stub_lookup_service() {
        let service = stub_lookup_service();
        let result = service.analyze("test input", 5).await;

        assert!(result.entities.is_empty());
        assert!(result.dominant_entity.is_none());
        assert!(!result.entities_resolved);
    }

    #[test]
    fn test_lookup_result_serializable() {
        let result = LookupResult {
            verbs: vec![],
            entities: vec![],
            dominant_entity: None,
            expected_kinds: vec!["company".to_string()],
            concepts: vec![],
            verb_matched: false,
            entities_resolved: false,
        };

        let json = serde_json::to_string(&result).expect("Should serialize");
        let parsed: LookupResult = serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(parsed.expected_kinds, result.expected_kinds);
    }
}
