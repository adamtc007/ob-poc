//! Pack-scoped entity resolution with focus, canonicalization, and exclusions
//!
//! Implements context-aware entity resolution per Phase D of the research
//! document. Resolution priority:
//!
//! 1. **Pronoun/focus** (zero cost) — "it", "that", "the manco"
//! 2. **Canonicalize** — "Luxembourg" → "LU", "S.A." → "SA"
//! 3. **Accumulated answers** — reuse answers from pack Q&A
//! 4. **Candidate universe** — scope ∩ expected_types ∩ pack_expected − exclusions
//! 5. **Type-dispatched search** — delegate to EntityArgResolver
//!
//! Invariant I-4: resolver CHOOSES from the candidate universe, never invents.

use std::collections::HashSet;

use uuid::Uuid;

use super::context_stack::{canonicalize_mention, ContextStack};
use super::types::{EntityCandidate, UnresolvedRef};

// ---------------------------------------------------------------------------
// Resolution outcome
// ---------------------------------------------------------------------------

/// Outcome of context-aware entity resolution.
#[derive(Debug, Clone)]
pub enum EntityResolutionOutcome {
    /// Resolved to a single entity via focus/pronoun.
    ResolvedByFocus {
        entity_id: Uuid,
        display_name: String,
        method: &'static str,
    },

    /// Resolved via accumulated answers from pack Q&A.
    ResolvedByAnswer {
        entity_id: Uuid,
        display_name: String,
        field: String,
    },

    /// Candidate universe built — delegate to search.
    NeedsSearch {
        canonicalized_input: String,
        expected_kinds: Vec<String>,
        candidate_universe: CandidateUniverse,
    },

    /// No candidates available — cannot resolve.
    NoMatch { reason: String },
}

// ---------------------------------------------------------------------------
// Candidate Universe (Invariant I-4)
// ---------------------------------------------------------------------------

/// The bounded universe of candidates for entity resolution.
///
/// `candidates = (scope_entities ∩ expected_types ∩ pack_expected) − exclusions`
///
/// The resolver CHOOSES from this set, never invents.
#[derive(Debug, Clone, Default)]
pub struct CandidateUniverse {
    /// Entity types allowed for this arg slot.
    pub expected_kinds: HashSet<String>,

    /// Scope constraint — entity must be related to these CBUs.
    pub scope_cbu_ids: Vec<Uuid>,

    /// Entity IDs explicitly excluded by user.
    pub excluded_entity_ids: HashSet<Uuid>,

    /// Excluded values (text-based).
    pub excluded_values: HashSet<String>,

    /// Pack's dominant domain (for type preference).
    pub pack_domain: Option<String>,

    /// Whether the universe is constrained (vs. open search).
    pub is_constrained: bool,
}

impl CandidateUniverse {
    /// Check if a candidate should be included.
    pub fn accepts(&self, candidate: &EntityCandidate) -> bool {
        // Exclusion filter
        if self.excluded_entity_ids.contains(&candidate.entity_id) {
            return false;
        }
        if self.excluded_values.contains(&candidate.name) {
            return false;
        }

        // Kind filter (if constrained)
        if !self.expected_kinds.is_empty() {
            if let Some(ref kind) = candidate.kind {
                if !self.expected_kinds.contains(kind) {
                    return false;
                }
            }
            // If candidate has no kind, allow it through (benefit of the doubt)
        }

        true
    }

    /// Filter a list of candidates through the universe constraints.
    pub fn filter(&self, candidates: Vec<EntityCandidate>) -> Vec<EntityCandidate> {
        candidates.into_iter().filter(|c| self.accepts(c)).collect()
    }
}

// ---------------------------------------------------------------------------
// resolve_with_context
// ---------------------------------------------------------------------------

/// Attempt to resolve an entity reference using context before falling back
/// to search.
///
/// Priority:
/// 1. Focus/pronoun resolution (zero cost)
/// 2. Canonicalization
/// 3. Accumulated answers
/// 4. Build candidate universe for search delegation
pub fn resolve_with_context(
    input: &str,
    expected_kinds: &[String],
    context: &ContextStack,
) -> EntityResolutionOutcome {
    // Step 1: Try pronoun/focus resolution
    if let Some(focus_ref) = context.focus.resolve_pronoun(input) {
        // Validate that the focus entity matches expected kinds
        if expected_kinds.is_empty() || expected_kinds.iter().any(|k| k == &focus_ref.entity_type) {
            return EntityResolutionOutcome::ResolvedByFocus {
                entity_id: focus_ref.id,
                display_name: focus_ref.display_name.clone(),
                method: "pronoun",
            };
        }
    }

    // Step 2: Canonicalize the input
    let canonicalized = canonicalize_mention(input);

    // Step 3: Try accumulated answers
    if let Some(value) = context.accumulated_answers.get(&canonicalized) {
        if let Some(uuid_str) = value.as_str() {
            if let Ok(entity_id) = Uuid::parse_str(uuid_str) {
                return EntityResolutionOutcome::ResolvedByAnswer {
                    entity_id,
                    display_name: canonicalized.clone(),
                    field: canonicalized,
                };
            }
        }
    }

    // Also check if input matches an answer field name
    let lower_input = input.to_lowercase();
    for (field, value) in &context.accumulated_answers {
        if field.to_lowercase() == lower_input {
            if let Some(uuid_str) = value.as_str() {
                if let Ok(entity_id) = Uuid::parse_str(uuid_str) {
                    return EntityResolutionOutcome::ResolvedByAnswer {
                        entity_id,
                        display_name: field.clone(),
                        field: field.clone(),
                    };
                }
            }
        }
    }

    // Step 4: Build candidate universe for search delegation
    let universe = build_candidate_universe(expected_kinds, context);

    if universe.is_constrained
        && universe.scope_cbu_ids.is_empty()
        && universe.expected_kinds.is_empty()
    {
        return EntityResolutionOutcome::NoMatch {
            reason: "No scope or type constraints available for entity resolution".to_string(),
        };
    }

    EntityResolutionOutcome::NeedsSearch {
        canonicalized_input: canonicalized,
        expected_kinds: expected_kinds.to_vec(),
        candidate_universe: universe,
    }
}

/// Build the candidate universe from context.
///
/// Invariant I-4:
/// `candidates = (scope_entities ∩ expected_types ∩ pack_expected) − exclusions`
pub fn build_candidate_universe(
    expected_kinds: &[String],
    context: &ContextStack,
) -> CandidateUniverse {
    let expected_kinds_set: HashSet<String> = expected_kinds.iter().cloned().collect();
    let scope_cbu_ids = context.derived_scope.loaded_cbu_ids.clone();

    let mut excluded_entity_ids = HashSet::new();
    let mut excluded_values = HashSet::new();
    for exclusion in &context.exclusions.exclusions {
        if let Some(id) = exclusion.entity_id {
            excluded_entity_ids.insert(id);
        }
        excluded_values.insert(exclusion.value.clone());
    }

    let pack_domain = context
        .active_pack()
        .and_then(|p| p.dominant_domain.clone());

    let is_constrained = !expected_kinds_set.is_empty()
        || !scope_cbu_ids.is_empty()
        || !excluded_entity_ids.is_empty();

    CandidateUniverse {
        expected_kinds: expected_kinds_set,
        scope_cbu_ids,
        excluded_entity_ids,
        excluded_values,
        pack_domain,
        is_constrained,
    }
}

/// Apply candidate universe filtering to unresolved refs.
///
/// Filters candidates in each UnresolvedRef through the universe
/// constraints, removing excluded or type-mismatched candidates.
pub fn filter_unresolved_refs(refs: &mut [UnresolvedRef], context: &ContextStack) {
    for uref in refs.iter_mut() {
        let expected_kinds: Vec<String> = uref
            .expected_kind
            .as_ref()
            .map(|k| vec![k.clone()])
            .unwrap_or_default();

        let universe = build_candidate_universe(&expected_kinds, context);

        // Filter candidates through universe
        uref.candidates.retain(|c| universe.accepts(c));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repl::context_stack::ContextStack;
    use crate::repl::runbook::Runbook;

    fn empty_context() -> ContextStack {
        let rb = Runbook::new(Uuid::new_v4());
        ContextStack::from_runbook(&rb, None, 0)
    }

    // -- resolve_with_context tests --

    #[test]
    fn test_resolve_pronoun_it() {
        let mut ctx = empty_context();
        let entity_id = Uuid::new_v4();
        ctx.focus.set_entity(
            entity_id,
            "Allianz SE".to_string(),
            "company".to_string(),
            1,
        );

        match resolve_with_context("it", &[], &ctx) {
            EntityResolutionOutcome::ResolvedByFocus {
                entity_id: id,
                method,
                ..
            } => {
                assert_eq!(id, entity_id);
                assert_eq!(method, "pronoun");
            }
            other => panic!("Expected ResolvedByFocus, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_pronoun_the_fund() {
        let mut ctx = empty_context();
        let cbu_id = Uuid::new_v4();
        ctx.focus
            .set_cbu(cbu_id, "Allianz Lux SICAV".to_string(), 1);

        match resolve_with_context("the fund", &[], &ctx) {
            EntityResolutionOutcome::ResolvedByFocus { entity_id: id, .. } => {
                assert_eq!(id, cbu_id);
            }
            other => panic!("Expected ResolvedByFocus, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_pronoun_type_mismatch_falls_through() {
        let mut ctx = empty_context();
        let entity_id = Uuid::new_v4();
        ctx.focus
            .set_entity(entity_id, "John Smith".to_string(), "person".to_string(), 1);

        // Expected kind is "company" but focus is "person" → falls through to search
        match resolve_with_context("it", &["company".to_string()], &ctx) {
            EntityResolutionOutcome::NeedsSearch { .. } => {} // expected
            other => panic!("Expected NeedsSearch, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_accumulated_answer() {
        let mut ctx = empty_context();
        let entity_id = Uuid::new_v4();
        ctx.accumulated_answers.insert(
            "depositary".to_string(),
            serde_json::Value::String(entity_id.to_string()),
        );

        match resolve_with_context("depositary", &[], &ctx) {
            EntityResolutionOutcome::ResolvedByAnswer {
                entity_id: id,
                field,
                ..
            } => {
                assert_eq!(id, entity_id);
                assert_eq!(field, "depositary");
            }
            other => panic!("Expected ResolvedByAnswer, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_canonicalization_before_search() {
        let ctx = empty_context();

        match resolve_with_context("Luxembourg", &["jurisdiction".to_string()], &ctx) {
            EntityResolutionOutcome::NeedsSearch {
                canonicalized_input,
                ..
            } => {
                assert_eq!(canonicalized_input, "LU");
            }
            other => panic!("Expected NeedsSearch, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_falls_through_to_search() {
        let ctx = empty_context();

        match resolve_with_context("Goldman Sachs", &["company".to_string()], &ctx) {
            EntityResolutionOutcome::NeedsSearch {
                canonicalized_input,
                expected_kinds,
                ..
            } => {
                assert_eq!(canonicalized_input, "Goldman Sachs");
                assert_eq!(expected_kinds, vec!["company".to_string()]);
            }
            other => panic!("Expected NeedsSearch, got {:?}", other),
        }
    }

    // -- build_candidate_universe tests --

    #[test]
    fn test_candidate_universe_with_scope() {
        let mut ctx = empty_context();
        let cbu1 = Uuid::new_v4();
        let cbu2 = Uuid::new_v4();
        ctx.derived_scope.loaded_cbu_ids = vec![cbu1, cbu2];

        let universe = build_candidate_universe(&["company".to_string()], &ctx);

        assert!(universe.is_constrained);
        assert_eq!(universe.scope_cbu_ids.len(), 2);
        assert!(universe.expected_kinds.contains("company"));
    }

    #[test]
    fn test_candidate_universe_excludes_rejected() {
        let mut ctx = empty_context();
        let excluded_id = Uuid::new_v4();
        ctx.exclusions.add_from_rejection(
            "Goldman Sachs".to_string(),
            Some(excluded_id),
            0,
            "wrong entity".to_string(),
        );

        let universe = build_candidate_universe(&[], &ctx);

        assert!(universe.excluded_entity_ids.contains(&excluded_id));
        assert!(universe.excluded_values.contains("Goldman Sachs"));
    }

    #[test]
    fn test_candidate_universe_accepts_valid_candidate() {
        let ctx = empty_context();
        let universe = build_candidate_universe(&["company".to_string()], &ctx);

        let candidate = EntityCandidate {
            entity_id: Uuid::new_v4(),
            name: "Allianz SE".to_string(),
            kind: Some("company".to_string()),
            score: 0.9,
        };

        assert!(universe.accepts(&candidate));
    }

    #[test]
    fn test_candidate_universe_rejects_wrong_kind() {
        let ctx = empty_context();
        let universe = build_candidate_universe(&["company".to_string()], &ctx);

        let candidate = EntityCandidate {
            entity_id: Uuid::new_v4(),
            name: "John Smith".to_string(),
            kind: Some("person".to_string()),
            score: 0.9,
        };

        assert!(!universe.accepts(&candidate));
    }

    #[test]
    fn test_candidate_universe_rejects_excluded_entity() {
        let mut ctx = empty_context();
        let excluded_id = Uuid::new_v4();
        ctx.exclusions.add_from_rejection(
            "Bad Corp".to_string(),
            Some(excluded_id),
            0,
            "rejected".to_string(),
        );

        let universe = build_candidate_universe(&[], &ctx);

        let candidate = EntityCandidate {
            entity_id: excluded_id,
            name: "Bad Corp".to_string(),
            kind: Some("company".to_string()),
            score: 0.9,
        };

        assert!(!universe.accepts(&candidate));
    }

    #[test]
    fn test_candidate_universe_filter_batch() {
        let mut ctx = empty_context();
        let excluded_id = Uuid::new_v4();
        ctx.exclusions.add_from_rejection(
            "Excluded".to_string(),
            Some(excluded_id),
            0,
            "rejected".to_string(),
        );

        let universe = build_candidate_universe(&["company".to_string()], &ctx);

        let candidates = vec![
            EntityCandidate {
                entity_id: Uuid::new_v4(),
                name: "Good Corp".to_string(),
                kind: Some("company".to_string()),
                score: 0.9,
            },
            EntityCandidate {
                entity_id: excluded_id,
                name: "Excluded".to_string(),
                kind: Some("company".to_string()),
                score: 0.85,
            },
            EntityCandidate {
                entity_id: Uuid::new_v4(),
                name: "John Person".to_string(),
                kind: Some("person".to_string()),
                score: 0.8,
            },
        ];

        let filtered = universe.filter(candidates);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Good Corp");
    }

    // -- filter_unresolved_refs tests --

    #[test]
    fn test_filter_unresolved_refs_removes_excluded() {
        let mut ctx = empty_context();
        let excluded_id = Uuid::new_v4();
        let good_id = Uuid::new_v4();
        ctx.exclusions.add_from_rejection(
            "Bad Entity".to_string(),
            Some(excluded_id),
            0,
            "rejected".to_string(),
        );

        let mut refs = vec![UnresolvedRef {
            ref_id: "ref-1".to_string(),
            text: "some entity".to_string(),
            expected_kind: Some("company".to_string()),
            candidates: vec![
                EntityCandidate {
                    entity_id: good_id,
                    name: "Good Entity".to_string(),
                    kind: Some("company".to_string()),
                    score: 0.9,
                },
                EntityCandidate {
                    entity_id: excluded_id,
                    name: "Bad Entity".to_string(),
                    kind: Some("company".to_string()),
                    score: 0.85,
                },
            ],
        }];

        filter_unresolved_refs(&mut refs, &ctx);

        assert_eq!(refs[0].candidates.len(), 1);
        assert_eq!(refs[0].candidates[0].entity_id, good_id);
    }
}
