//! Tier-gate decision API — Tranche 3 Phase 3.C (2026-04-26).
//!
//! Convenience wrapper that consumes the v1.1 / v1.2 helpers
//! ([`compute_effective_tier_with_trace`] +
//! [`compute_runbook_tier_with_trace`]) and produces a
//! `TierGateDecision` containing both the *tier* and the *human-readable
//! reason* — exactly what Sage / REPL need to render their preview UX
//! per `docs/policies/sage_autonomy.md` and `docs/policies/repl_confirmation.md`.
//!
//! No runtime side effects; callers (orchestrator / Sage proposer / REPL
//! confirmation prompt) consume `TierGateDecision::recommended_action`
//! to decide whether to execute, prompt, or refuse.
//!
//! ## Example
//!
//! ```ignore
//! use dsl_core::config::tier_gate::{TierGateDecision, TierGateAction};
//! use dsl_core::config::escalation::EvaluationContext;
//!
//! let ctx = EvaluationContext::new()
//!     .with_arg("amount", serde_json::json!(100_000_000));
//!
//! let decision = TierGateDecision::for_verb(verb_config, &ctx);
//! match decision.recommended_action {
//!     TierGateAction::Execute => orchestrator.dispatch(),
//!     TierGateAction::Announce(msg) => repl.announce(&msg).then(orchestrator.dispatch),
//!     TierGateAction::Confirm(prompt) => repl.confirm(&prompt).then(orchestrator.dispatch),
//!     TierGateAction::AuthorisePhrase(prompt, phrase) => repl.typed_confirm(&prompt, &phrase),
//! }
//! ```

use crate::config::escalation::{compute_effective_tier_with_trace, EvaluationContext};
use crate::config::runbook_composition::{
    compute_runbook_tier_with_trace, AggregationRule, CrossScopeRule, RunbookStep, RunbookTierTrace,
};
use crate::config::types::{ConsequenceTier, VerbConfig};

/// What a Sage / REPL caller should DO when a verb / runbook is invoked
/// at the computed effective tier. Maps directly to the policy documents
/// (`docs/policies/sage_autonomy.md`, `docs/policies/repl_confirmation.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TierGateAction {
    /// `benign` — execute without prompt.
    Execute,
    /// `reviewable` — execute with a brief announcement / preview line.
    /// String is the announcement text.
    Announce(String),
    /// `requires_confirmation` — pause for `[y/N]` confirmation.
    /// String is the prompt text.
    Confirm(String),
    /// `requires_explicit_authorisation` — pause for typed paraphrase
    /// confirmation. First string is the prompt; second is the
    /// expected phrase the user must type.
    AuthorisePhrase {
        prompt: String,
        expected_phrase: String,
    },
}

/// Decision produced by the tier gate. Carries the effective tier, the
/// recommended action, and a human-readable explanation of *why* the
/// tier was assigned (escalation rules + composition reasons).
#[derive(Debug, Clone)]
pub struct TierGateDecision {
    /// Computed effective tier (post-escalation, post-composition).
    pub effective_tier: ConsequenceTier,
    /// Baseline tier from the verb declaration (or max-step-tier for runbook).
    pub baseline_tier: ConsequenceTier,
    /// Recommended action for Sage / REPL.
    pub recommended_action: TierGateAction,
    /// Human-readable explanation. Empty when effective_tier == baseline_tier
    /// (no escalation fired); non-empty when escalation rules or composition
    /// rules raised the tier above baseline.
    pub explanation: String,
    /// Names of escalation rules that fired. Empty for runbook decisions
    /// where the elevation came from composition not per-step escalation.
    pub fired_rules: Vec<String>,
}

impl TierGateDecision {
    /// Compute the decision for a single-verb invocation.
    pub fn for_verb(verb: &VerbConfig, ctx: &EvaluationContext) -> Self {
        let Some(decl) = verb.three_axis.as_ref() else {
            // No three-axis → benign by default. Conservative; the
            // validator should already have caught undeclared verbs.
            return Self {
                effective_tier: ConsequenceTier::Benign,
                baseline_tier: ConsequenceTier::Benign,
                recommended_action: TierGateAction::Execute,
                explanation: String::new(),
                fired_rules: Vec::new(),
            };
        };
        let (effective_tier, fired) = compute_effective_tier_with_trace(&decl.consequence, ctx);
        let baseline_tier = decl.consequence.baseline;
        let fired_rules: Vec<String> = fired.iter().map(|r| r.name.clone()).collect();
        let explanation = if fired_rules.is_empty() {
            String::new()
        } else {
            let reasons: Vec<String> = fired
                .iter()
                .map(|r| {
                    format!(
                        "{} → {:?}{}",
                        r.name,
                        r.tier,
                        r.reason
                            .as_ref()
                            .map(|s| format!(" ({})", s))
                            .unwrap_or_default()
                    )
                })
                .collect();
            format!(
                "Tier raised from {:?} to {:?} by escalation rules: [{}]",
                baseline_tier,
                effective_tier,
                reasons.join("; ")
            )
        };
        let recommended_action = action_for_tier(effective_tier, &explanation);
        Self {
            effective_tier,
            baseline_tier,
            recommended_action,
            explanation,
            fired_rules,
        }
    }

    /// Compute the decision for a runbook (macro-expanded or ad-hoc).
    /// Per v1.2 P12, the composition is the max of: max-step-tier,
    /// aggregation rule tier (if any matches), cross-scope rule tier
    /// (if any matches).
    pub fn for_runbook(
        steps: &[RunbookStep],
        aggregation_rules: &[AggregationRule],
        cross_scope_rules: &[CrossScopeRule],
    ) -> Self {
        let trace: RunbookTierTrace =
            compute_runbook_tier_with_trace(steps, aggregation_rules, cross_scope_rules);
        let baseline_tier = trace.component_a; // max-step-tier
        let effective_tier = trace.effective;
        let mut explanation_parts = Vec::new();
        if effective_tier > baseline_tier {
            if !trace.aggregation_fired.is_empty() {
                explanation_parts.push(format!(
                    "aggregation rules fired: [{}]",
                    trace.aggregation_fired.join(", ")
                ));
            }
            if !trace.cross_scope_fired.is_empty() {
                explanation_parts.push(format!(
                    "cross-scope rules fired: [{}]",
                    trace.cross_scope_fired.join(", ")
                ));
            }
        }
        let explanation = if explanation_parts.is_empty() {
            String::new()
        } else {
            format!(
                "Composed tier {:?} (max-step {:?}); {}",
                effective_tier,
                baseline_tier,
                explanation_parts.join(", ")
            )
        };
        let recommended_action = action_for_tier(effective_tier, &explanation);
        Self {
            effective_tier,
            baseline_tier,
            recommended_action,
            explanation,
            fired_rules: Vec::new(),
        }
    }
}

fn action_for_tier(tier: ConsequenceTier, explanation: &str) -> TierGateAction {
    match tier {
        ConsequenceTier::Benign => TierGateAction::Execute,
        ConsequenceTier::Reviewable => {
            let msg = if explanation.is_empty() {
                "Reviewable action — proceeding".to_string()
            } else {
                format!("Reviewable action — proceeding. {}", explanation)
            };
            TierGateAction::Announce(msg)
        }
        ConsequenceTier::RequiresConfirmation => {
            let prompt = if explanation.is_empty() {
                "Confirm this action? [y/N]".to_string()
            } else {
                format!("Confirm this action ({})? [y/N]", explanation)
            };
            TierGateAction::Confirm(prompt)
        }
        ConsequenceTier::RequiresExplicitAuthorisation => {
            let prompt = if explanation.is_empty() {
                "This action requires explicit authorisation. Type the confirmation phrase to proceed:".to_string()
            } else {
                format!(
                    "This action requires explicit authorisation. {}. Type the confirmation phrase to proceed:",
                    explanation
                )
            };
            TierGateAction::AuthorisePhrase {
                prompt,
                // Default expected phrase: "I AUTHORISE". Callers may
                // override with a more specific phrase derived from the
                // verb's args (e.g. "deal-1234 CONTRACTED" for a deal
                // contracted authorisation).
                expected_phrase: "I AUTHORISE".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{ConsequenceDeclaration, EscalationPredicate, EscalationRule};

    fn benign_decl() -> ConsequenceDeclaration {
        ConsequenceDeclaration {
            baseline: ConsequenceTier::Benign,
            escalation: vec![],
        }
    }

    fn make_verb(decl: ConsequenceDeclaration) -> VerbConfig {
        // Parse a minimal YAML so we don't have to construct VerbConfig
        // by-hand (it has many internal fields without Default).
        let baseline_str = match decl.baseline {
            ConsequenceTier::Benign => "benign",
            ConsequenceTier::Reviewable => "reviewable",
            ConsequenceTier::RequiresConfirmation => "requires_confirmation",
            ConsequenceTier::RequiresExplicitAuthorisation => "requires_explicit_authorisation",
        };
        let yaml = format!(
            r#"description: t
behavior: plugin
three_axis:
  state_effect: preserving
  external_effects: []
  consequence:
    baseline: {baseline_str}
"#
        );
        let mut verb: VerbConfig = serde_yaml::from_str(&yaml).expect("parses");
        if let Some(ta) = verb.three_axis.as_mut() {
            ta.consequence.escalation = decl.escalation;
        }
        verb
    }

    #[test]
    fn benign_verb_recommends_execute() {
        let verb = make_verb(benign_decl());
        let ctx = EvaluationContext::new();
        let decision = TierGateDecision::for_verb(&verb, &ctx);
        assert_eq!(decision.effective_tier, ConsequenceTier::Benign);
        assert!(matches!(
            decision.recommended_action,
            TierGateAction::Execute
        ));
        assert!(decision.explanation.is_empty());
    }

    #[test]
    fn reviewable_verb_recommends_announce() {
        let verb = make_verb(ConsequenceDeclaration {
            baseline: ConsequenceTier::Reviewable,
            escalation: vec![],
        });
        let decision = TierGateDecision::for_verb(&verb, &EvaluationContext::new());
        assert!(matches!(
            decision.recommended_action,
            TierGateAction::Announce(_)
        ));
    }

    #[test]
    fn confirm_verb_recommends_confirm() {
        let verb = make_verb(ConsequenceDeclaration {
            baseline: ConsequenceTier::RequiresConfirmation,
            escalation: vec![],
        });
        let decision = TierGateDecision::for_verb(&verb, &EvaluationContext::new());
        assert!(matches!(
            decision.recommended_action,
            TierGateAction::Confirm(_)
        ));
    }

    #[test]
    fn auth_verb_recommends_typed_paraphrase() {
        let verb = make_verb(ConsequenceDeclaration {
            baseline: ConsequenceTier::RequiresExplicitAuthorisation,
            escalation: vec![],
        });
        let decision = TierGateDecision::for_verb(&verb, &EvaluationContext::new());
        assert!(matches!(
            decision.recommended_action,
            TierGateAction::AuthorisePhrase { .. }
        ));
    }

    #[test]
    fn escalation_raises_tier_and_populates_explanation() {
        let decl = ConsequenceDeclaration {
            baseline: ConsequenceTier::Reviewable,
            escalation: vec![EscalationRule {
                name: "large_amount".into(),
                when: EscalationPredicate::ArgGt {
                    arg: "amount".into(),
                    value: 50_000_000.0,
                },
                tier: ConsequenceTier::RequiresExplicitAuthorisation,
                reason: Some("amount exceeds operator sign-off threshold".into()),
            }],
        };
        let verb = make_verb(decl);
        let ctx = EvaluationContext::new().with_arg("amount", serde_json::json!(100_000_000));
        let decision = TierGateDecision::for_verb(&verb, &ctx);
        assert_eq!(
            decision.effective_tier,
            ConsequenceTier::RequiresExplicitAuthorisation
        );
        assert_eq!(decision.baseline_tier, ConsequenceTier::Reviewable);
        assert!(decision.explanation.contains("large_amount"));
        assert_eq!(decision.fired_rules, vec!["large_amount".to_string()]);
        assert!(matches!(
            decision.recommended_action,
            TierGateAction::AuthorisePhrase { .. }
        ));
    }

    #[test]
    fn no_three_axis_defaults_to_benign() {
        let yaml = "description: t\nbehavior: plugin\n";
        let verb: VerbConfig = serde_yaml::from_str(yaml).expect("parses");
        assert!(verb.three_axis.is_none());
        let decision = TierGateDecision::for_verb(&verb, &EvaluationContext::new());
        assert_eq!(decision.effective_tier, ConsequenceTier::Benign);
        assert!(matches!(
            decision.recommended_action,
            TierGateAction::Execute
        ));
    }
}
