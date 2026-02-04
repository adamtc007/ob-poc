//! Unified LookupService - Verb-first dual search
//!
//! Combines verb discovery and entity resolution in a single pass,
//! using verb schema to constrain entity kinds.

use crate::entity_linking::{EntityLinkingService, EntityResolution, StubEntityLinkingService};
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

    /// Create with all components
    pub fn with_all(
        entity_linker: Arc<dyn EntityLinkingService>,
        lexicon: Arc<dyn LexiconService>,
        verb_searcher: Arc<HybridVerbSearcher>,
    ) -> Self {
        Self {
            entity_linker,
            lexicon: Some(lexicon),
            verb_searcher: Some(verb_searcher),
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

    /// Analyze utterance with verb-first ordering
    ///
    /// 1. Search verbs to find likely intent
    /// 2. Derive expected entity kinds from verb schema
    /// 3. Resolve entities with kind constraints
    pub async fn analyze(&self, utterance: &str, limit: usize) -> LookupResult {
        // Step 1: Verb search (verb-first ordering)
        let verbs = if let Some(searcher) = &self.verb_searcher {
            searcher
                .search(utterance, None, None, limit)
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

        // Step 4: Find dominant entity
        let dominant_entity = self.find_dominant(&entities);
        let has_dominant = dominant_entity.is_some();

        LookupResult {
            verbs,
            entities,
            dominant_entity,
            expected_kinds,
            concepts: vec![], // Concepts not extracted in this version
            verb_matched,
            entities_resolved: has_dominant,
        }
    }

    /// Analyze with pre-known expected kinds (skip verb search)
    pub fn analyze_entities_only(
        &self,
        utterance: &str,
        expected_kinds: &[String],
        limit: usize,
    ) -> LookupResult {
        let entities = self.entity_linker.resolve_mentions(
            utterance,
            if expected_kinds.is_empty() {
                None
            } else {
                Some(expected_kinds)
            },
            None,
            limit,
        );

        let dominant_entity = self.find_dominant(&entities);
        let has_dominant = dominant_entity.is_some();

        LookupResult {
            verbs: vec![],
            entities,
            dominant_entity,
            expected_kinds: expected_kinds.to_vec(),
            concepts: vec![],
            verb_matched: false,
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

/// Create a stub lookup service for testing/graceful degradation
pub fn stub_lookup_service() -> LookupService {
    LookupService::new(Arc::new(StubEntityLinkingService))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_lookup_service() {
        let service = stub_lookup_service();
        let result = service.analyze_entities_only("test input", &[], 5);

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
