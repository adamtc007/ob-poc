//! Runbook consequence-tier composition (v1.1 P12 — pilot P.1.e).
//!
//! Implements the uniform runbook-level effective-tier formula from v1.1 P12:
//!
//! ```text
//! runbook_tier = max(
//!     max(step.effective_tier for step in runbook),        // Component A
//!     aggregation_tier_if_applicable(runbook),             // Component B (default benign)
//!     cross_scope_tier_if_applicable(runbook)              // Component C (default benign)
//! )
//! ```
//!
//! Applies uniformly to macro-expanded runbooks and ad-hoc REPL-assembled
//! runbooks (P12 invariant) — the composition function doesn't care which.
//!
//! Pure function: no DB, no HTTP, no wall clock. Composes over an input
//! slice of `RunbookStep` records where each step's `effective_tier` has
//! already been computed by `config::escalation::compute_effective_tier`
//! (P.1.b).
//!
//! This module is the catalogue-side composition engine only. Runtime
//! evaluation (the Sage/REPL path that combines verb-level escalation +
//! this composition) wires in later pilot phases.

use crate::config::types::{ConsequenceTier, ExternalEffect, StateEffect};
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Composition-layer step record
// ---------------------------------------------------------------------------

/// A composition-layer view of one runbook step. Populated by the Sage /
/// REPL dispatcher after per-verb `effective_tier` is computed and before
/// composition applies.
///
/// `RunbookStep` is intentionally minimal — only the fields the three
/// composition components need. The verb's full declaration is upstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunbookStep {
    /// Verb FQN. Enables traceability; not used in tier math directly.
    pub verb_fqn: String,
    /// Effective tier after per-verb escalation rules applied.
    pub effective_tier: ConsequenceTier,
    /// State effect from the verb's three-axis declaration.
    pub state_effect: StateEffect,
    /// External-effects set — used by Component B patterns that count
    /// effect occurrences.
    pub external_effects: Vec<ExternalEffect>,
    /// Workspace the step operates in (e.g. "instrument_matrix", "cbu").
    /// Used by Component C for multi-workspace detection.
    pub workspace: String,
    /// DAG the step transitions (if any). `None` for state-preserving
    /// steps. Used by Component C for multi-DAG detection.
    pub dag: Option<String>,
    /// Entity kind the step touches (e.g. "cbu", "deal", "kyc_case").
    /// Used by Component C for cross-entity-kind detection.
    pub entity_kind: Option<String>,
}

// ---------------------------------------------------------------------------
// Component B — aggregation rules (per v1.1 P12)
// ---------------------------------------------------------------------------

/// A catalogue-declared aggregation pattern. Fires when a runbook matches
/// the pattern (bulk cardinality, repeated effects, high-volume updates).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregationRule {
    /// Runbook has at least `threshold` steps total.
    BulkCardinality {
        name: String,
        threshold: usize,
        tier: ConsequenceTier,
    },
    /// Runbook has at least `threshold` steps that produce the named
    /// external effect. Typical use: `RepeatedExternalEffect { effect:
    /// Emitting, threshold: 5 }` to catch "this runbook emits 5+ signals".
    RepeatedExternalEffect {
        name: String,
        effect: ExternalEffect,
        threshold: usize,
        tier: ConsequenceTier,
    },
    /// Runbook has at least `threshold` state-transition steps
    /// (high-volume updates pattern).
    TransitionCount {
        name: String,
        threshold: usize,
        tier: ConsequenceTier,
    },
}

impl AggregationRule {
    pub fn name(&self) -> &str {
        match self {
            Self::BulkCardinality { name, .. }
            | Self::RepeatedExternalEffect { name, .. }
            | Self::TransitionCount { name, .. } => name,
        }
    }

    pub fn tier(&self) -> ConsequenceTier {
        match self {
            Self::BulkCardinality { tier, .. }
            | Self::RepeatedExternalEffect { tier, .. }
            | Self::TransitionCount { tier, .. } => *tier,
        }
    }

    pub fn matches(&self, steps: &[RunbookStep]) -> bool {
        match self {
            Self::BulkCardinality { threshold, .. } => steps.len() >= *threshold,
            Self::RepeatedExternalEffect {
                effect, threshold, ..
            } => steps
                .iter()
                .filter(|s| s.external_effects.contains(effect))
                .count()
                >= *threshold,
            Self::TransitionCount { threshold, .. } => steps
                .iter()
                .filter(|s| s.state_effect == StateEffect::Transition)
                .count()
                >= *threshold,
        }
    }
}

// ---------------------------------------------------------------------------
// Component C — cross-scope rules (per v1.1 P12)
// ---------------------------------------------------------------------------

/// A catalogue-declared cross-scope pattern. Fires when a runbook spans
/// multiple scopes (workspaces, DAGs, entity kinds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossScopeRule {
    /// Runbook touches at least `min_workspaces` distinct workspaces.
    MultiWorkspace {
        name: String,
        min_workspaces: usize,
        tier: ConsequenceTier,
    },
    /// Runbook transitions at least `min_dags` distinct DAGs.
    MultiDag {
        name: String,
        min_dags: usize,
        tier: ConsequenceTier,
    },
    /// Runbook touches at least `min_kinds` distinct entity kinds.
    MultiEntityKind {
        name: String,
        min_kinds: usize,
        tier: ConsequenceTier,
    },
}

impl CrossScopeRule {
    pub fn name(&self) -> &str {
        match self {
            Self::MultiWorkspace { name, .. }
            | Self::MultiDag { name, .. }
            | Self::MultiEntityKind { name, .. } => name,
        }
    }

    pub fn tier(&self) -> ConsequenceTier {
        match self {
            Self::MultiWorkspace { tier, .. }
            | Self::MultiDag { tier, .. }
            | Self::MultiEntityKind { tier, .. } => *tier,
        }
    }

    pub fn matches(&self, steps: &[RunbookStep]) -> bool {
        match self {
            Self::MultiWorkspace { min_workspaces, .. } => {
                let distinct: HashSet<&str> =
                    steps.iter().map(|s| s.workspace.as_str()).collect();
                distinct.len() >= *min_workspaces
            }
            Self::MultiDag { min_dags, .. } => {
                let distinct: HashSet<&str> = steps
                    .iter()
                    .filter_map(|s| s.dag.as_deref())
                    .collect();
                distinct.len() >= *min_dags
            }
            Self::MultiEntityKind { min_kinds, .. } => {
                let distinct: HashSet<&str> = steps
                    .iter()
                    .filter_map(|s| s.entity_kind.as_deref())
                    .collect();
                distinct.len() >= *min_kinds
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Composition entry points
// ---------------------------------------------------------------------------

/// Compute the runbook's effective tier per v1.1 P12:
/// `max(Component A, Component B, Component C)`.
///
/// - Component A (mandatory): `max(step.effective_tier)` — defaults to
///   `Benign` for an empty runbook.
/// - Component B (optional): max tier of matching aggregation rules —
///   defaults to `Benign` when no rules match.
/// - Component C (optional): max tier of matching cross-scope rules —
///   defaults to `Benign` when no rules match.
pub fn compute_runbook_tier(
    steps: &[RunbookStep],
    aggregation: &[AggregationRule],
    cross_scope: &[CrossScopeRule],
) -> ConsequenceTier {
    let a = component_a(steps);
    let b = component_b(steps, aggregation);
    let c = component_c(steps, cross_scope);
    a.max(b).max(c)
}

/// Like `compute_runbook_tier` but also returns each component's tier and
/// the names of the rules that fired. Drives the UX transparency required
/// by v1.1 Open Question 15.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunbookTierTrace {
    pub effective: ConsequenceTier,
    pub component_a: ConsequenceTier,
    pub component_b: ConsequenceTier,
    pub component_c: ConsequenceTier,
    /// Names of aggregation rules that fired, in declaration order.
    pub aggregation_fired: Vec<String>,
    /// Names of cross-scope rules that fired, in declaration order.
    pub cross_scope_fired: Vec<String>,
}

pub fn compute_runbook_tier_with_trace(
    steps: &[RunbookStep],
    aggregation: &[AggregationRule],
    cross_scope: &[CrossScopeRule],
) -> RunbookTierTrace {
    let a = component_a(steps);
    let aggregation_fired: Vec<_> = aggregation
        .iter()
        .filter(|r| r.matches(steps))
        .map(|r| r.name().to_string())
        .collect();
    let b = aggregation
        .iter()
        .filter(|r| r.matches(steps))
        .map(|r| r.tier())
        .fold(ConsequenceTier::Benign, ConsequenceTier::max);
    let cross_scope_fired: Vec<_> = cross_scope
        .iter()
        .filter(|r| r.matches(steps))
        .map(|r| r.name().to_string())
        .collect();
    let c = cross_scope
        .iter()
        .filter(|r| r.matches(steps))
        .map(|r| r.tier())
        .fold(ConsequenceTier::Benign, ConsequenceTier::max);

    RunbookTierTrace {
        effective: a.max(b).max(c),
        component_a: a,
        component_b: b,
        component_c: c,
        aggregation_fired,
        cross_scope_fired,
    }
}

// ---------------------------------------------------------------------------
// Component implementations (exposed for targeted unit tests)
// ---------------------------------------------------------------------------

pub fn component_a(steps: &[RunbookStep]) -> ConsequenceTier {
    steps
        .iter()
        .map(|s| s.effective_tier)
        .fold(ConsequenceTier::Benign, ConsequenceTier::max)
}

pub fn component_b(steps: &[RunbookStep], rules: &[AggregationRule]) -> ConsequenceTier {
    rules
        .iter()
        .filter(|r| r.matches(steps))
        .map(|r| r.tier())
        .fold(ConsequenceTier::Benign, ConsequenceTier::max)
}

pub fn component_c(steps: &[RunbookStep], rules: &[CrossScopeRule]) -> ConsequenceTier {
    rules
        .iter()
        .filter(|r| r.matches(steps))
        .map(|r| r.tier())
        .fold(ConsequenceTier::Benign, ConsequenceTier::max)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_step(
        fqn: &str,
        tier: ConsequenceTier,
        state: StateEffect,
        workspace: &str,
    ) -> RunbookStep {
        RunbookStep {
            verb_fqn: fqn.into(),
            effective_tier: tier,
            state_effect: state,
            external_effects: vec![],
            workspace: workspace.into(),
            dag: None,
            entity_kind: None,
        }
    }

    // --- Component A ---

    #[test]
    fn component_a_empty_runbook_is_benign() {
        assert_eq!(component_a(&[]), ConsequenceTier::Benign);
    }

    #[test]
    fn component_a_returns_max_step_tier() {
        let steps = vec![
            mk_step("a.low", ConsequenceTier::Benign, StateEffect::Preserving, "w"),
            mk_step("a.mid", ConsequenceTier::Reviewable, StateEffect::Preserving, "w"),
            mk_step(
                "a.high",
                ConsequenceTier::RequiresConfirmation,
                StateEffect::Preserving,
                "w",
            ),
        ];
        assert_eq!(component_a(&steps), ConsequenceTier::RequiresConfirmation);
    }

    // --- Component B ---

    #[test]
    fn component_b_bulk_cardinality_fires_on_threshold() {
        let rule = AggregationRule::BulkCardinality {
            name: "bulk".into(),
            threshold: 5,
            tier: ConsequenceTier::Reviewable,
        };
        let below = (0..4)
            .map(|_| mk_step("x", ConsequenceTier::Benign, StateEffect::Preserving, "w"))
            .collect::<Vec<_>>();
        let at = (0..5)
            .map(|_| mk_step("x", ConsequenceTier::Benign, StateEffect::Preserving, "w"))
            .collect::<Vec<_>>();
        assert_eq!(component_b(&below, std::slice::from_ref(&rule)), ConsequenceTier::Benign);
        assert_eq!(component_b(&at, &[rule]), ConsequenceTier::Reviewable);
    }

    #[test]
    fn component_b_repeated_external_effect() {
        let rule = AggregationRule::RepeatedExternalEffect {
            name: "emits".into(),
            effect: ExternalEffect::Emitting,
            threshold: 3,
            tier: ConsequenceTier::RequiresConfirmation,
        };
        let mk_emitting = |tier| {
            let mut s = mk_step("e", tier, StateEffect::Preserving, "w");
            s.external_effects = vec![ExternalEffect::Emitting];
            s
        };
        let mk_silent = |tier| mk_step("s", tier, StateEffect::Preserving, "w");
        // 2 emitting, 3 silent → does NOT fire.
        let below = vec![
            mk_emitting(ConsequenceTier::Benign),
            mk_emitting(ConsequenceTier::Benign),
            mk_silent(ConsequenceTier::Benign),
            mk_silent(ConsequenceTier::Benign),
            mk_silent(ConsequenceTier::Benign),
        ];
        // 3 emitting → fires.
        let at = vec![
            mk_emitting(ConsequenceTier::Benign),
            mk_emitting(ConsequenceTier::Benign),
            mk_emitting(ConsequenceTier::Benign),
        ];
        assert_eq!(component_b(&below, std::slice::from_ref(&rule)), ConsequenceTier::Benign);
        assert_eq!(
            component_b(&at, &[rule]),
            ConsequenceTier::RequiresConfirmation
        );
    }

    #[test]
    fn component_b_transition_count() {
        let rule = AggregationRule::TransitionCount {
            name: "many_transitions".into(),
            threshold: 2,
            tier: ConsequenceTier::Reviewable,
        };
        let steps = vec![
            mk_step("t1", ConsequenceTier::Benign, StateEffect::Transition, "w"),
            mk_step("p1", ConsequenceTier::Benign, StateEffect::Preserving, "w"),
            mk_step("t2", ConsequenceTier::Benign, StateEffect::Transition, "w"),
        ];
        assert_eq!(component_b(&steps, &[rule]), ConsequenceTier::Reviewable);
    }

    #[test]
    fn component_b_max_over_multiple_rules() {
        let rules = vec![
            AggregationRule::BulkCardinality {
                name: "bulk".into(),
                threshold: 2,
                tier: ConsequenceTier::Reviewable,
            },
            AggregationRule::BulkCardinality {
                name: "huge".into(),
                threshold: 1,
                tier: ConsequenceTier::RequiresConfirmation,
            },
        ];
        let steps = vec![
            mk_step("a", ConsequenceTier::Benign, StateEffect::Preserving, "w"),
            mk_step("b", ConsequenceTier::Benign, StateEffect::Preserving, "w"),
        ];
        // Both rules match; max tier wins.
        assert_eq!(
            component_b(&steps, &rules),
            ConsequenceTier::RequiresConfirmation
        );
    }

    // --- Component C ---

    #[test]
    fn component_c_multi_workspace_fires() {
        let rule = CrossScopeRule::MultiWorkspace {
            name: "cross_ws".into(),
            min_workspaces: 2,
            tier: ConsequenceTier::RequiresConfirmation,
        };
        let same_ws = vec![
            mk_step("a", ConsequenceTier::Benign, StateEffect::Preserving, "w1"),
            mk_step("b", ConsequenceTier::Benign, StateEffect::Preserving, "w1"),
        ];
        let cross_ws = vec![
            mk_step("a", ConsequenceTier::Benign, StateEffect::Preserving, "w1"),
            mk_step("b", ConsequenceTier::Benign, StateEffect::Preserving, "w2"),
        ];
        assert_eq!(
            component_c(&same_ws, std::slice::from_ref(&rule)),
            ConsequenceTier::Benign
        );
        assert_eq!(
            component_c(&cross_ws, &[rule]),
            ConsequenceTier::RequiresConfirmation
        );
    }

    #[test]
    fn component_c_multi_dag_ignores_stateless_steps() {
        let rule = CrossScopeRule::MultiDag {
            name: "cross_dag".into(),
            min_dags: 2,
            tier: ConsequenceTier::Reviewable,
        };
        let mut s1 = mk_step("a", ConsequenceTier::Benign, StateEffect::Transition, "w");
        s1.dag = Some("d1".into());
        let mut s2 = mk_step("b", ConsequenceTier::Benign, StateEffect::Transition, "w");
        s2.dag = Some("d2".into());
        let stateless = mk_step("s", ConsequenceTier::Benign, StateEffect::Preserving, "w");
        // 2 distinct DAGs → fires.
        assert_eq!(
            component_c(&[s1.clone(), s2.clone(), stateless.clone()], std::slice::from_ref(&rule)),
            ConsequenceTier::Reviewable
        );
        // Only 1 DAG → doesn't fire.
        assert_eq!(
            component_c(&[s1, stateless], &[rule]),
            ConsequenceTier::Benign
        );
    }

    #[test]
    fn component_c_multi_entity_kind() {
        let rule = CrossScopeRule::MultiEntityKind {
            name: "cross_kind".into(),
            min_kinds: 2,
            tier: ConsequenceTier::Reviewable,
        };
        let mut s1 = mk_step("a", ConsequenceTier::Benign, StateEffect::Preserving, "w");
        s1.entity_kind = Some("cbu".into());
        let mut s2 = mk_step("b", ConsequenceTier::Benign, StateEffect::Preserving, "w");
        s2.entity_kind = Some("deal".into());
        assert_eq!(component_c(&[s1, s2], &[rule]), ConsequenceTier::Reviewable);
    }

    // --- Composed ---

    #[test]
    fn compose_empty_runbook_is_benign() {
        assert_eq!(compute_runbook_tier(&[], &[], &[]), ConsequenceTier::Benign);
    }

    #[test]
    fn compose_takes_max_of_three_components() {
        let steps = vec![mk_step(
            "single",
            ConsequenceTier::Reviewable,
            StateEffect::Preserving,
            "w",
        )];
        let agg = vec![AggregationRule::BulkCardinality {
            name: "bulk".into(),
            threshold: 1,
            tier: ConsequenceTier::RequiresConfirmation,
        }];
        let xs = vec![CrossScopeRule::MultiWorkspace {
            name: "x".into(),
            min_workspaces: 1,
            tier: ConsequenceTier::RequiresExplicitAuthorisation,
        }];
        // A=Reviewable, B=RequiresConfirmation, C=RequiresExplicitAuthorisation
        // → C wins.
        assert_eq!(
            compute_runbook_tier(&steps, &agg, &xs),
            ConsequenceTier::RequiresExplicitAuthorisation
        );
    }

    #[test]
    fn trace_records_component_tiers_and_fired_rules() {
        let steps = vec![
            mk_step("s1", ConsequenceTier::Reviewable, StateEffect::Transition, "w1"),
            mk_step("s2", ConsequenceTier::Benign, StateEffect::Transition, "w2"),
        ];
        let agg = vec![AggregationRule::TransitionCount {
            name: "many_t".into(),
            threshold: 2,
            tier: ConsequenceTier::RequiresConfirmation,
        }];
        let xs = vec![CrossScopeRule::MultiWorkspace {
            name: "cross".into(),
            min_workspaces: 2,
            tier: ConsequenceTier::Reviewable,
        }];
        let trace = compute_runbook_tier_with_trace(&steps, &agg, &xs);
        assert_eq!(trace.component_a, ConsequenceTier::Reviewable);
        assert_eq!(trace.component_b, ConsequenceTier::RequiresConfirmation);
        assert_eq!(trace.component_c, ConsequenceTier::Reviewable);
        assert_eq!(trace.effective, ConsequenceTier::RequiresConfirmation);
        assert_eq!(trace.aggregation_fired, vec!["many_t"]);
        assert_eq!(trace.cross_scope_fired, vec!["cross"]);
    }

    #[test]
    fn macro_and_adhoc_runbooks_use_same_composition() {
        // P12 invariant: composition applies uniformly regardless of
        // runbook origin. This is tested by showing that two runbooks
        // with identical step-sequences produce identical tiers,
        // regardless of how they were assembled (macro-expansion vs
        // REPL typing — the composition function doesn't distinguish).
        let macro_expanded = vec![
            mk_step("a", ConsequenceTier::Reviewable, StateEffect::Preserving, "w"),
            mk_step("b", ConsequenceTier::Reviewable, StateEffect::Preserving, "w"),
        ];
        let adhoc_assembled = macro_expanded.clone();
        let rules = vec![AggregationRule::BulkCardinality {
            name: "bulk".into(),
            threshold: 2,
            tier: ConsequenceTier::RequiresConfirmation,
        }];
        let t1 = compute_runbook_tier(&macro_expanded, &rules, &[]);
        let t2 = compute_runbook_tier(&adhoc_assembled, &rules, &[]);
        assert_eq!(t1, t2);
        assert_eq!(t1, ConsequenceTier::RequiresConfirmation);
    }
}
