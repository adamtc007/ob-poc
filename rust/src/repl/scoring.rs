//! Pack-scoped verb scoring and ambiguity policy
//!
//! This module implements the re-ranking layer from §5.3 of the research
//! document. Raw semantic search scores are adjusted based on pack context:
//!
//! - **Forbidden** verbs → score zeroed
//! - **In-pack** verbs → boosted
//! - **Out-of-pack** verbs → penalised (mildly — strong semantic match still wins)
//! - **Template step** verb → strongly boosted
//! - **Domain affinity** → small boost for verbs sharing the pack's domain
//!
//! All constants are extracted here so they can be calibrated via the
//! tuning harness (Phase G).

use super::context_stack::ContextStack;
use super::types::VerbCandidate;

// ---------------------------------------------------------------------------
// Scoring Constants
// ---------------------------------------------------------------------------

/// Boost applied to verbs in the active pack's allowed set.
pub const PACK_VERB_BOOST: f32 = 0.10;

/// Penalty applied to verbs NOT in the active pack (and not forbidden).
pub const PACK_VERB_PENALTY: f32 = 0.05;

/// Boost applied to the verb matching the next expected template step.
pub const TEMPLATE_STEP_BOOST: f32 = 0.15;

/// Small boost for verbs whose domain matches the pack's dominant domain.
pub const DOMAIN_AFFINITY_BOOST: f32 = 0.03;

/// Absolute score floor — candidates below this are dropped.
pub const ABSOLUTE_FLOOR: f32 = 0.55;

/// Decision threshold — top candidate must be above this to be considered.
pub const THRESHOLD: f32 = 0.55;

/// Ambiguity margin — if top - runner-up < MARGIN, it's ambiguous.
pub const MARGIN: f32 = 0.05;

/// Strong threshold — above this, no disambiguation needed regardless of margin.
pub const STRONG_THRESHOLD: f32 = 0.70;

// ---------------------------------------------------------------------------
// Scoring Functions
// ---------------------------------------------------------------------------

/// Apply pack-scoped scoring adjustments to a list of verb candidates.
///
/// Mutates scores in-place and re-sorts by adjusted score (descending).
/// Candidates with forbidden verbs are removed entirely.
///
/// If no pack is active, candidates are returned unchanged (except for
/// floor filtering).
pub fn apply_pack_scoring(candidates: &mut Vec<VerbCandidate>, context: &ContextStack) {
    let pack = context.active_pack();

    candidates.retain_mut(|c| {
        if let Some(pack) = pack {
            // Forbidden → remove entirely
            if pack.forbidden_verbs.contains(&c.verb_fqn) {
                return false;
            }

            // In-pack boost
            if pack.allowed_verbs.contains(&c.verb_fqn) {
                c.score += PACK_VERB_BOOST;
            } else if !pack.allowed_verbs.is_empty() {
                // Out-of-pack penalty (only if pack has an allowed set)
                c.score -= PACK_VERB_PENALTY;
            }

            // Template step boost
            if context.is_template_step(&c.verb_fqn) {
                c.score += TEMPLATE_STEP_BOOST;
            }

            // Domain affinity boost
            if let Some(ref dominant_domain) = pack.dominant_domain {
                if let Some(verb_domain) = c.verb_fqn.split('.').next() {
                    if verb_domain == dominant_domain {
                        c.score += DOMAIN_AFFINITY_BOOST;
                    }
                }
            }
        } else {
            // No pack active — apply focus-mode domain boost from recent verbs.
            let focus_mode = super::context_stack::derive_focus_mode(context);
            if let Some(focus_domain) = focus_mode.domain() {
                if let Some(verb_domain) = c.verb_fqn.split('.').next() {
                    if verb_domain == focus_domain {
                        c.score += DOMAIN_AFFINITY_BOOST;
                    }
                }
            }
        }

        // Exclusion filtering
        if context.exclusions.is_excluded(&c.verb_fqn, None) {
            return false;
        }

        // Floor filtering
        c.score >= ABSOLUTE_FLOOR
    });

    // Re-sort by adjusted score (descending)
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

// ---------------------------------------------------------------------------
// Ambiguity Policy (Invariant I-5)
// ---------------------------------------------------------------------------

/// Outcome of ambiguity analysis after scoring.
#[derive(Debug, Clone, PartialEq)]
pub enum AmbiguityOutcome {
    /// No candidates survived scoring.
    NoMatch,

    /// Clear winner — top candidate is confident.
    Confident { verb: String, score: f32 },

    /// Two candidates too close — need user to pick.
    Ambiguous {
        top: VerbCandidate,
        runner_up: VerbCandidate,
        margin: f32,
    },

    /// Top candidate is above threshold but not strong — propose with lower confidence.
    Proposed { verb: String, score: f32 },
}

/// Apply the uniform ambiguity policy to scored candidates.
///
/// Rules:
/// 1. No candidates → `NoMatch`
/// 2. Top candidate ≥ STRONG_THRESHOLD → `Confident` (regardless of margin)
/// 3. Only 1 candidate ≥ THRESHOLD → `Confident`
/// 4. Top 2 candidates ≥ THRESHOLD with margin < MARGIN → `Ambiguous`
/// 5. Top candidate ≥ THRESHOLD → `Proposed`
/// 6. Otherwise → `NoMatch`
pub fn apply_ambiguity_policy(candidates: &[VerbCandidate]) -> AmbiguityOutcome {
    if candidates.is_empty() {
        return AmbiguityOutcome::NoMatch;
    }

    let top = &candidates[0];

    // Rule 2: Strong match — no disambiguation needed
    if top.score >= STRONG_THRESHOLD {
        return AmbiguityOutcome::Confident {
            verb: top.verb_fqn.clone(),
            score: top.score,
        };
    }

    // Below threshold entirely
    if top.score < THRESHOLD {
        return AmbiguityOutcome::NoMatch;
    }

    // Only one candidate above threshold
    if candidates.len() < 2 || candidates[1].score < THRESHOLD {
        return AmbiguityOutcome::Proposed {
            verb: top.verb_fqn.clone(),
            score: top.score,
        };
    }

    // Two candidates — check margin
    let runner_up = &candidates[1];
    let margin = top.score - runner_up.score;

    if margin < MARGIN {
        return AmbiguityOutcome::Ambiguous {
            top: top.clone(),
            runner_up: runner_up.clone(),
            margin,
        };
    }

    // Margin is sufficient — top wins
    AmbiguityOutcome::Proposed {
        verb: top.verb_fqn.clone(),
        score: top.score,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repl::context_stack::{ContextStack, PackContext, TemplateStepHint};
    use crate::repl::runbook::Runbook;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_candidate(verb: &str, score: f32) -> VerbCandidate {
        VerbCandidate {
            verb_fqn: verb.to_string(),
            description: format!("Test {}", verb),
            score,
            example: None,
            domain: verb.split('.').next().map(|s| s.to_string()),
        }
    }

    fn empty_context() -> ContextStack {
        let rb = Runbook::new(Uuid::new_v4());
        ContextStack::from_runbook(&rb, None, 0)
    }

    fn context_with_pack(
        allowed: Vec<&str>,
        forbidden: Vec<&str>,
        domain: Option<&str>,
    ) -> ContextStack {
        let rb = Runbook::new(Uuid::new_v4());
        let mut ctx = ContextStack::from_runbook(&rb, None, 0);
        ctx.pack_staged = Some(PackContext {
            pack_id: "test-pack".to_string(),
            pack_version: "1.0".to_string(),
            allowed_verbs: allowed.into_iter().map(|s| s.to_string()).collect(),
            forbidden_verbs: forbidden.into_iter().map(|s| s.to_string()).collect(),
            dominant_domain: domain.map(|s| s.to_string()),
            template_ids: vec![],
            invocation_phrases: vec![],
        });
        ctx
    }

    // -- apply_pack_scoring tests --

    #[test]
    fn test_forbidden_verbs_removed() {
        let ctx = context_with_pack(vec!["kyc.create-case"], vec!["cbu.delete"], Some("kyc"));
        let mut candidates = vec![
            make_candidate("kyc.create-case", 0.80),
            make_candidate("cbu.delete", 0.85),
        ];

        apply_pack_scoring(&mut candidates, &ctx);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].verb_fqn, "kyc.create-case");
    }

    #[test]
    fn test_in_pack_boost() {
        let ctx = context_with_pack(
            vec!["kyc.create-case", "kyc.add-entity"],
            vec![],
            Some("kyc"),
        );
        let mut candidates = vec![
            make_candidate("cbu.create", 0.75),
            make_candidate("kyc.create-case", 0.70),
        ];

        apply_pack_scoring(&mut candidates, &ctx);

        // kyc.create-case should now be higher due to boost + domain affinity
        assert_eq!(candidates[0].verb_fqn, "kyc.create-case");
        // 0.70 + 0.10 (pack) + 0.03 (domain) = 0.83
        assert!((candidates[0].score - 0.83).abs() < 0.001);
    }

    #[test]
    fn test_out_of_pack_penalty() {
        let ctx = context_with_pack(vec!["kyc.create-case"], vec![], Some("kyc"));
        let mut candidates = vec![make_candidate("cbu.create", 0.65)];

        apply_pack_scoring(&mut candidates, &ctx);

        // 0.65 - 0.05 (out-of-pack) = 0.60
        assert!((candidates[0].score - 0.60).abs() < 0.001);
    }

    #[test]
    fn test_template_step_boost() {
        let mut ctx = context_with_pack(
            vec!["kyc.create-case", "kyc.add-entity"],
            vec![],
            Some("kyc"),
        );
        ctx.template_hint = Some(TemplateStepHint {
            template_id: "standard-kyc".to_string(),
            step_index: 1,
            total_steps: 5,
            expected_verb: "kyc.add-entity".to_string(),
            next_entry_id: Uuid::new_v4(),
            section: None,
            section_progress: None,
            carry_forward_args: HashMap::new(),
        });

        let mut candidates = vec![
            make_candidate("kyc.create-case", 0.75),
            make_candidate("kyc.add-entity", 0.70),
        ];

        apply_pack_scoring(&mut candidates, &ctx);

        // kyc.add-entity: 0.70 + 0.10 (pack) + 0.15 (template) + 0.03 (domain) = 0.98
        assert_eq!(candidates[0].verb_fqn, "kyc.add-entity");
        assert!((candidates[0].score - 0.98).abs() < 0.001);
    }

    #[test]
    fn test_domain_affinity_boost() {
        let ctx = context_with_pack(vec![], vec![], Some("kyc"));
        let mut candidates = vec![
            make_candidate("kyc.list-cases", 0.60),
            make_candidate("cbu.list", 0.60),
        ];

        apply_pack_scoring(&mut candidates, &ctx);

        // kyc.list-cases gets +0.03 domain affinity
        assert_eq!(candidates[0].verb_fqn, "kyc.list-cases");
        assert!((candidates[0].score - 0.63).abs() < 0.001);
    }

    #[test]
    fn test_floor_filtering() {
        let ctx = empty_context();
        let mut candidates = vec![
            make_candidate("cbu.create", 0.80),
            make_candidate("cbu.delete", 0.40), // below floor
        ];

        apply_pack_scoring(&mut candidates, &ctx);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].verb_fqn, "cbu.create");
    }

    #[test]
    fn test_no_pack_passthrough() {
        let ctx = empty_context();
        let mut candidates = vec![
            make_candidate("cbu.create", 0.80),
            make_candidate("kyc.create-case", 0.75),
        ];

        apply_pack_scoring(&mut candidates, &ctx);

        // No pack → no boost/penalty, just floor filtering
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].verb_fqn, "cbu.create");
        assert!((candidates[0].score - 0.80).abs() < 0.001);
    }

    #[test]
    fn test_exclusion_filtering() {
        let mut ctx = empty_context();
        ctx.exclusions.add_from_rejection(
            "cbu.delete".to_string(),
            None,
            0,
            "user rejected".to_string(),
        );

        let mut candidates = vec![
            make_candidate("cbu.create", 0.80),
            make_candidate("cbu.delete", 0.75),
        ];

        apply_pack_scoring(&mut candidates, &ctx);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].verb_fqn, "cbu.create");
    }

    // -- apply_ambiguity_policy tests --

    #[test]
    fn test_ambiguity_no_candidates() {
        assert_eq!(apply_ambiguity_policy(&[]), AmbiguityOutcome::NoMatch);
    }

    #[test]
    fn test_ambiguity_strong_match() {
        let candidates = vec![
            make_candidate("kyc.create-case", 0.85),
            make_candidate("cbu.create", 0.82),
        ];
        match apply_ambiguity_policy(&candidates) {
            AmbiguityOutcome::Confident { verb, score } => {
                assert_eq!(verb, "kyc.create-case");
                assert!((score - 0.85).abs() < 0.001);
            }
            other => panic!("Expected Confident, got {:?}", other),
        }
    }

    #[test]
    fn test_ambiguity_two_close_candidates() {
        let candidates = vec![
            make_candidate("session.load-galaxy", 0.65),
            make_candidate("session.load-cbu", 0.63),
        ];
        match apply_ambiguity_policy(&candidates) {
            AmbiguityOutcome::Ambiguous {
                top,
                runner_up,
                margin,
            } => {
                assert_eq!(top.verb_fqn, "session.load-galaxy");
                assert_eq!(runner_up.verb_fqn, "session.load-cbu");
                assert!(margin < MARGIN);
            }
            other => panic!("Expected Ambiguous, got {:?}", other),
        }
    }

    #[test]
    fn test_ambiguity_proposed() {
        let candidates = vec![
            make_candidate("cbu.create", 0.62),
            make_candidate("cbu.list", 0.55),
        ];
        match apply_ambiguity_policy(&candidates) {
            AmbiguityOutcome::Proposed { verb, score } => {
                assert_eq!(verb, "cbu.create");
                assert!((score - 0.62).abs() < 0.001);
            }
            other => panic!("Expected Proposed, got {:?}", other),
        }
    }

    #[test]
    fn test_ambiguity_below_threshold() {
        let candidates = vec![make_candidate("cbu.create", 0.50)];
        assert_eq!(
            apply_ambiguity_policy(&candidates),
            AmbiguityOutcome::NoMatch
        );
    }

    #[test]
    fn test_ambiguity_single_above_threshold() {
        let candidates = vec![
            make_candidate("cbu.create", 0.60),
            make_candidate("cbu.delete", 0.50), // below threshold
        ];
        match apply_ambiguity_policy(&candidates) {
            AmbiguityOutcome::Proposed { verb, .. } => {
                assert_eq!(verb, "cbu.create");
            }
            other => panic!("Expected Proposed, got {:?}", other),
        }
    }

    #[test]
    fn test_ambiguity_margin_sufficient() {
        let candidates = vec![
            make_candidate("kyc.create-case", 0.68),
            make_candidate("kyc.add-entity", 0.60), // margin = 0.08 > MARGIN
        ];
        match apply_ambiguity_policy(&candidates) {
            AmbiguityOutcome::Proposed { verb, .. } => {
                assert_eq!(verb, "kyc.create-case");
            }
            other => panic!("Expected Proposed, got {:?}", other),
        }
    }
}
