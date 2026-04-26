//! Three-axis declaration validator (v1.1 §6.2 — pilot P.1.c / P.1.d).
//!
//! Pure function library: takes a `VerbsConfig` (or a single `VerbConfig`)
//! and returns a list of errors and warnings. No DB access (P3). No
//! ordering assumptions — callers may aggregate across the whole catalogue.
//!
//! v1.1 §6.2 defines three error classes:
//!
//! - **Structural errors** — the declaration is internally inconsistent at
//!   the mechanical level (e.g. `state_effect: transition` without a
//!   `transitions` block).
//! - **Well-formedness errors** — the declaration references names that
//!   don't exist in the rest of the verb (e.g. an escalation predicate
//!   names an arg that isn't in the verb's `args:` list).
//! - **Policy-sanity warnings** — conservative, narrow, raised ONLY for
//!   combinations that are mechanically internally inconsistent. P10's
//!   orthogonality means MOST "unusual" combinations are legitimate
//!   (state-preserving + `requires_explicit_authorisation`, state-transition
//!   + `external_effects: []` + `requires_explicit_authorisation`) and the
//!     validator stays silent. Warnings are for the narrow mechanically-broken
//!     set — not opinion.
//!
//! P.1.c implements structural + well-formedness. P.1.d adds the policy-
//! sanity warnings.

use crate::config::types::{
    ConsequenceDeclaration, ConsequenceTier, EscalationPredicate, ExternalEffect, StateEffect,
    ThreeAxisDeclaration, VerbConfig, VerbsConfig,
};
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Error + warning taxonomy
// ---------------------------------------------------------------------------

/// Location of a finding — verb-scoped where known, catalogue-wide otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// `domain.verb` FQN if known, else some other identifier.
    pub fqn: String,
    /// Optional path fragment within the verb's declaration for UX.
    pub path: Option<String>,
}

impl Location {
    pub fn verb(fqn: impl Into<String>) -> Self {
        Self {
            fqn: fqn.into(),
            path: None,
        }
    }
    pub fn verb_path(fqn: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            fqn: fqn.into(),
            path: Some(path.into()),
        }
    }
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.path {
            Some(p) => write!(f, "{}::{}", self.fqn, p),
            None => f.write_str(&self.fqn),
        }
    }
}

/// Structural errors — the declaration is mechanically inconsistent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralError {
    /// `state_effect: transition` declared without a `transitions:` block
    /// OR with an empty `transitions.edges` list.
    TransitionWithoutEdges(Location),
    /// `state_effect: preserving` declared together with a non-empty
    /// `transitions:` block. P10's orthogonality holds but the transitions
    /// block is only meaningful for transition-effect verbs.
    PreservingWithTransitions(Location),
}

impl std::fmt::Display for StructuralError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransitionWithoutEdges(loc) => write!(
                f,
                "{loc}: state_effect=transition requires a non-empty transitions.edges list"
            ),
            Self::PreservingWithTransitions(loc) => write!(
                f,
                "{loc}: state_effect=preserving must not declare a transitions block"
            ),
        }
    }
}

/// Well-formedness errors — the declaration references names that don't
/// exist in the rest of the verb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WellFormednessError {
    /// Escalation rule's predicate references an argument name that is not
    /// declared in the verb's `args:` list. Rule index + predicate rendered
    /// for operator UX.
    EscalationArgNotDeclared {
        location: Location,
        rule_name: String,
        arg: String,
    },
    /// Escalation rule's tier is strictly below the declared baseline.
    /// P11 says rules can only raise tier — a rule whose tier is `< baseline`
    /// is dead code at best, a bug at worst.
    EscalationTierBelowBaseline {
        location: Location,
        rule_name: String,
        rule_tier: ConsequenceTier,
        baseline: ConsequenceTier,
    },
    /// Declaration is missing on a verb that should carry one. The
    /// `expected` flag is set by callers who know the workspace has been
    /// migrated; during rollout, callers may run with `require_declaration:
    /// false` so missing declarations are tolerated.
    DeclarationIncomplete { location: Location },
    /// Transitions block declares a `dag:` name that doesn't match any
    /// known DAG taxonomy. Checked only when the caller passes a known-DAG
    /// set; otherwise skipped (P.2 produces the taxonomy; P.1.c alone
    /// can't cross-check).
    UnknownDagReference {
        location: Location,
        dag: String,
    },
    /// Two or more escalation rules within the same verb share a name.
    /// Names must be unique per verb so audit-trail records of which rule
    /// fired are unambiguous.
    DuplicateRuleName {
        location: Location,
        rule_name: String,
        occurrences: usize,
    },
    /// A pack file references a verb FQN that isn't declared in any verb
    /// YAML nor as a macro. Typically caused by pack-authoring against an
    /// *expected* verb surface that was never YAML-implemented (pilot A-2
    /// found 11 such cases in instrument-matrix.yaml). Catching this at
    /// catalogue-load time prevents drift accumulation across packs
    /// workspace-wide.
    PackFqnWithoutDeclaration {
        pack_name: String,
        fqn: String,
    },
}

impl std::fmt::Display for WellFormednessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EscalationArgNotDeclared {
                location,
                rule_name,
                arg,
            } => write!(
                f,
                "{location}: escalation rule '{rule_name}' references arg '{arg}' which is not \
                 declared in the verb's args list"
            ),
            Self::EscalationTierBelowBaseline {
                location,
                rule_name,
                rule_tier,
                baseline,
            } => write!(
                f,
                "{location}: escalation rule '{rule_name}' tier ({rule_tier:?}) is below \
                 baseline ({baseline:?}); rules can only raise tier per v1.1 P11"
            ),
            Self::DeclarationIncomplete { location } => {
                write!(f, "{location}: missing three_axis declaration (v1.1 P1)")
            }
            Self::UnknownDagReference { location, dag } => {
                write!(
                    f,
                    "{location}: transitions.dag='{dag}' does not match any known DAG taxonomy"
                )
            }
            Self::DuplicateRuleName {
                location,
                rule_name,
                occurrences,
            } => write!(
                f,
                "{location}: escalation rule name '{rule_name}' appears {occurrences} times — \
                 rule names must be unique per verb for audit-trail determinism"
            ),
            Self::PackFqnWithoutDeclaration { pack_name, fqn } => write!(
                f,
                "pack '{pack_name}' references verb '{fqn}' which is not declared in any \
                 verb YAML or macro — pack entry is stale or verb is missing"
            ),
        }
    }
}

/// Conservative policy-sanity warnings (P.1.d will populate this).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyWarning {
    /// The escalation predicate is definitionally unreachable (e.g. a
    /// predicate whose tier equals the baseline — always dominated).
    /// Populated by P.1.d.
    UnreachableEscalation {
        location: Location,
        rule_name: String,
    },
}

impl std::fmt::Display for PolicyWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnreachableEscalation {
                location,
                rule_name,
            } => write!(
                f,
                "{location}: escalation rule '{rule_name}' tier equals baseline — rule is a \
                 no-op (warning only, not a bug)"
            ),
        }
    }
}

/// Validator output.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    pub structural: Vec<StructuralError>,
    pub well_formedness: Vec<WellFormednessError>,
    pub warnings: Vec<PolicyWarning>,
}

impl ValidationReport {
    pub fn is_clean(&self) -> bool {
        self.structural.is_empty() && self.well_formedness.is_empty()
    }
    pub fn error_count(&self) -> usize {
        self.structural.len() + self.well_formedness.len()
    }
}

// ---------------------------------------------------------------------------
// Validator entry points
// ---------------------------------------------------------------------------

/// Optional cross-reference information. If provided, the validator
/// additionally checks `transitions.dag` references against known DAGs.
#[derive(Debug, Clone, Default)]
pub struct ValidationContext {
    /// Set of DAG taxonomy names the catalogue knows about. If empty, DAG
    /// cross-checks are skipped.
    pub known_dags: HashSet<String>,
    /// Whether verbs MUST carry a declaration. During rollout, set false so
    /// missing declarations don't error (but are reported via
    /// `DeclarationIncomplete` if caller opts in).
    pub require_declaration: bool,
}

/// Validate a single verb. Returns a report — callers aggregate across the
/// catalogue.
pub fn validate_verb(
    fqn: &str,
    verb: &VerbConfig,
    ctx: &ValidationContext,
) -> ValidationReport {
    let mut report = ValidationReport::default();

    match &verb.three_axis {
        None => {
            if ctx.require_declaration {
                report
                    .well_formedness
                    .push(WellFormednessError::DeclarationIncomplete {
                        location: Location::verb(fqn),
                    });
            }
        }
        Some(decl) => validate_declaration(fqn, verb, decl, ctx, &mut report),
    }

    report
}

fn validate_declaration(
    fqn: &str,
    verb: &VerbConfig,
    decl: &ThreeAxisDeclaration,
    ctx: &ValidationContext,
    report: &mut ValidationReport,
) {
    // Structural: transition ↔ non-empty transitions.
    match (decl.state_effect, &decl.transitions) {
        (StateEffect::Transition, Some(t)) if !t.edges.is_empty() => {
            // OK. Optional: cross-check DAG reference if the context has one.
            if !ctx.known_dags.is_empty() && !ctx.known_dags.contains(&t.dag) {
                report
                    .well_formedness
                    .push(WellFormednessError::UnknownDagReference {
                        location: Location::verb_path(fqn, "transitions.dag"),
                        dag: t.dag.clone(),
                    });
            }
        }
        (StateEffect::Transition, _) => {
            report
                .structural
                .push(StructuralError::TransitionWithoutEdges(Location::verb(fqn)));
        }
        (StateEffect::Preserving, Some(t)) if !t.edges.is_empty() => {
            report
                .structural
                .push(StructuralError::PreservingWithTransitions(Location::verb(
                    fqn,
                )));
        }
        (StateEffect::Preserving, _) => { /* OK */ }
    }

    // Well-formedness: escalation predicates reference declared args.
    let declared_args: HashSet<String> =
        verb.args.iter().map(|a| a.name.clone()).collect();
    validate_consequence(
        fqn,
        &decl.consequence,
        &declared_args,
        report,
    );

    // P10 sanity note: state-preserving + any consequence tier is legal.
    // State-transition + empty external_effects + RequiresExplicitAuthorisation
    // is legal (sanctions transitions, settlement-readiness advances). No
    // warnings raised here — see P.1.d for the narrow warning set.
    let _ = (&decl.external_effects,); // explicitly noted: no check fires here
}

fn validate_consequence(
    fqn: &str,
    conseq: &ConsequenceDeclaration,
    declared_args: &HashSet<String>,
    report: &mut ValidationReport,
) {
    // --- well-formedness: duplicate rule names (P.1.d) ---
    // Rule names must be unique per verb so audit-trail records are
    // unambiguous about which rule fired. Flag any name that appears > 1.
    let mut name_counts: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    for rule in &conseq.escalation {
        *name_counts.entry(rule.name.as_str()).or_insert(0) += 1;
    }
    // Emit one error per duplicated name (not per occurrence) — keeps the
    // report concise.
    let mut reported_dups: HashSet<String> = HashSet::new();
    for rule in &conseq.escalation {
        let count = name_counts[rule.name.as_str()];
        if count > 1 && reported_dups.insert(rule.name.clone()) {
            report
                .well_formedness
                .push(WellFormednessError::DuplicateRuleName {
                    location: Location::verb_path(fqn, "consequence.escalation"),
                    rule_name: rule.name.clone(),
                    occurrences: count,
                });
        }
    }

    for rule in &conseq.escalation {
        // Tier monotonicity (P11): a rule's tier must be >= baseline, else
        // it's dead code. We treat `<` as a well-formedness error.
        if rule.tier < conseq.baseline {
            report
                .well_formedness
                .push(WellFormednessError::EscalationTierBelowBaseline {
                    location: Location::verb_path(fqn, "consequence.escalation"),
                    rule_name: rule.name.clone(),
                    rule_tier: rule.tier,
                    baseline: conseq.baseline,
                });
        }
        // --- policy-sanity warning: unreachable escalation (P.1.d) ---
        // A rule whose tier equals baseline can never change the effective
        // tier via max(). It's a no-op — not a bug (unlike tier < baseline
        // which is a bug), but worth flagging so authors consolidate rules.
        // Narrow warning per v1.1 §6.2 — mechanically-dead rule only;
        // does not opine on "should this be higher."
        if rule.tier == conseq.baseline {
            report.warnings.push(PolicyWarning::UnreachableEscalation {
                location: Location::verb_path(fqn, "consequence.escalation"),
                rule_name: rule.name.clone(),
            });
        }
        // Arg references.
        let mut referenced_args: HashSet<String> = HashSet::new();
        collect_predicate_arg_refs(&rule.when, &mut referenced_args);
        for arg in referenced_args {
            if !declared_args.contains(&arg) {
                report
                    .well_formedness
                    .push(WellFormednessError::EscalationArgNotDeclared {
                        location: Location::verb_path(fqn, "consequence.escalation"),
                        rule_name: rule.name.clone(),
                        arg,
                    });
            }
        }
    }
}

/// Collect every `arg` name referenced by a predicate (transitively through
/// and / or / not).
fn collect_predicate_arg_refs(pred: &EscalationPredicate, acc: &mut HashSet<String>) {
    match pred {
        EscalationPredicate::ArgEq { arg, .. }
        | EscalationPredicate::ArgIn { arg, .. }
        | EscalationPredicate::ArgGt { arg, .. }
        | EscalationPredicate::ArgGte { arg, .. }
        | EscalationPredicate::ArgLt { arg, .. }
        | EscalationPredicate::ArgLte { arg, .. } => {
            acc.insert(arg.clone());
        }
        EscalationPredicate::EntityAttrEq { .. }
        | EscalationPredicate::EntityAttrIn { .. }
        | EscalationPredicate::ContextFlag { .. } => { /* not arg refs */ }
        EscalationPredicate::And { preds } | EscalationPredicate::Or { preds } => {
            for p in preds {
                collect_predicate_arg_refs(p, acc);
            }
        }
        EscalationPredicate::Not { pred } => collect_predicate_arg_refs(pred, acc),
    }
}

/// Validate every verb in a `VerbsConfig`. Returns one aggregated report.
pub fn validate_verbs_config(
    config: &VerbsConfig,
    ctx: &ValidationContext,
) -> ValidationReport {
    let mut report = ValidationReport::default();
    for (domain_name, domain) in &config.domains {
        for (verb_name, verb) in &domain.verbs {
            let fqn = format!("{domain_name}.{verb_name}");
            let per = validate_verb(&fqn, verb, ctx);
            report.structural.extend(per.structural);
            report.well_formedness.extend(per.well_formedness);
            report.warnings.extend(per.warnings);
        }
    }
    report
}

/// Build the set of declared FQN strings from a `VerbsConfig`. Used by
/// pack-hygiene validation (see [`validate_pack_fqns`]) to distinguish
/// declared verbs from pack references.
///
/// Each entry is `domain_name.verb_name` — matches the format used in
/// pack `allowed_verbs:` lists.
pub fn collect_declared_fqns(config: &VerbsConfig) -> HashSet<String> {
    let mut out = HashSet::new();
    for (domain_name, domain) in &config.domains {
        for verb_name in domain.verbs.keys() {
            out.insert(format!("{domain_name}.{verb_name}"));
        }
    }
    out
}

/// V1.2-5 pack-hygiene check: verify every FQN in a pack's
/// `allowed_verbs` list resolves to either (a) a declared verb in
/// `VerbsConfig`, or (b) a macro FQN from the macro registry.
///
/// Callers provide:
/// - `declared_verbs`: set of `domain.verb_name` FQNs from
///   [`collect_declared_fqns`].
/// - `macro_fqns`: set of macro FQNs (from `config/verb_schemas/macros/`
///   YAML — loaded separately by the caller, since macros are a distinct
///   YAML surface).
/// - `pack_entries`: iterator of `(pack_name, fqn)` tuples from all pack
///   `allowed_verbs` lists.
///
/// Returns a list of [`WellFormednessError::PackFqnWithoutDeclaration`]
/// entries, one per pack FQN that doesn't resolve. Empty list = clean.
///
/// This was the class of bug A-2 found: `matrix-overlay.apply`,
/// `delivery.create`, etc. were listed in the Instrument Matrix pack
/// but never YAML-implemented. Running this check as part of
/// catalogue-load would catch this drift at author time.
pub fn validate_pack_fqns(
    declared_verbs: &HashSet<String>,
    macro_fqns: &HashSet<String>,
    pack_entries: impl IntoIterator<Item = (String, String)>,
) -> Vec<WellFormednessError> {
    let mut errors = Vec::new();
    for (pack_name, fqn) in pack_entries {
        if declared_verbs.contains(&fqn) || macro_fqns.contains(&fqn) {
            continue;
        }
        errors.push(WellFormednessError::PackFqnWithoutDeclaration {
            pack_name,
            fqn,
        });
    }
    errors
}

// Silence `unused` warnings for fields / variants reserved for P.1.d.
#[allow(dead_code)]
fn _reserved_for_p1_d(_: &ExternalEffect) {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{
        ArgConfig, ArgType, ConsequenceDeclaration, ConsequenceTier, EscalationPredicate,
        EscalationRule, StateEffect, ThreeAxisDeclaration, TransitionEdge, VerbBehavior,
        VerbConfig, VerbTransitions,
    };
    use serde_json::json;

    fn bare_verb_config() -> VerbConfig {
        VerbConfig {
            description: "test verb".into(),
            behavior: VerbBehavior::Plugin,
            crud: None,
            handler: None,
            graph_query: None,
            durable: None,
            args: vec![],
            returns: None,
            produces: None,
            consumes: vec![],
            lifecycle: None,
            metadata: None,
            invocation_phrases: vec![],
            policy: None,
            sentences: None,
            confirm_policy: None,
            outputs: vec![],
            three_axis: None,
            transition_args: None,
        }
    }

    fn arg(name: &str) -> ArgConfig {
        ArgConfig {
            name: name.into(),
            arg_type: ArgType::String,
            required: true,
            lookup: None,
            valid_values: None,
            default: None,
            description: None,
            validation: None,
            fuzzy_check: None,
            slot_type: None,
            preferred_roles: vec![],
            maps_to: None,
        }
    }

    #[test]
    fn declaration_absent_with_require_declaration_is_incomplete() {
        let vc = bare_verb_config();
        let ctx = ValidationContext {
            require_declaration: true,
            ..ValidationContext::default()
        };
        let r = validate_verb("test.verb", &vc, &ctx);
        assert_eq!(r.well_formedness.len(), 1);
        assert!(matches!(
            r.well_formedness[0],
            WellFormednessError::DeclarationIncomplete { .. }
        ));
    }

    #[test]
    fn declaration_absent_with_rollout_mode_is_silent() {
        let vc = bare_verb_config();
        let ctx = ValidationContext::default(); // require_declaration=false
        let r = validate_verb("test.verb", &vc, &ctx);
        assert!(r.is_clean());
    }

    #[test]
    fn transition_without_edges_is_structural_error() {
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Transition,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Benign,
                escalation: vec![],
            },
            transitions: None,
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        assert_eq!(r.structural.len(), 1);
        assert!(matches!(
            r.structural[0],
            StructuralError::TransitionWithoutEdges(_)
        ));
    }

    #[test]
    fn transition_with_empty_edges_also_structural() {
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Transition,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Benign,
                escalation: vec![],
            },
            transitions: Some(VerbTransitions {
                dag: "some_dag".into(),
                edges: vec![],
            }),
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        assert_eq!(r.structural.len(), 1);
    }

    #[test]
    fn preserving_with_transitions_is_structural_error() {
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Preserving,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Benign,
                escalation: vec![],
            },
            transitions: Some(VerbTransitions {
                dag: "d".into(),
                edges: vec![TransitionEdge {
                    from: "a".into(),
                    to: "b".into(),
                }],
            }),
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        assert_eq!(r.structural.len(), 1);
        assert!(matches!(
            r.structural[0],
            StructuralError::PreservingWithTransitions(_)
        ));
    }

    #[test]
    fn valid_transition_declaration_passes() {
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Transition,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Reviewable,
                escalation: vec![],
            },
            transitions: Some(VerbTransitions {
                dag: "some_dag".into(),
                edges: vec![TransitionEdge {
                    from: "draft".into(),
                    to: "submitted".into(),
                }],
            }),
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        assert!(r.is_clean());
    }

    #[test]
    fn p10_state_preserving_plus_requires_explicit_authorisation_is_legal() {
        // v1.1 §6.2 explicitly: state-preserving verbs with
        // requires_explicit_authorisation (exports, attestations,
        // disclosures) pass silently. No warning fires.
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Preserving,
            external_effects: vec![ExternalEffect::Emitting],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::RequiresExplicitAuthorisation,
                escalation: vec![],
            },
            transitions: None,
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        assert!(r.is_clean(), "P10 orthogonality — legal combination");
    }

    #[test]
    fn p10_state_transition_plus_no_external_plus_high_tier_is_legal() {
        // v1.1 §6.2 explicitly: state-transition + external_effects: [] +
        // requires_explicit_authorisation (sanctions-state transitions,
        // settlement-readiness advances, approval-state changes) passes.
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Transition,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::RequiresExplicitAuthorisation,
                escalation: vec![],
            },
            transitions: Some(VerbTransitions {
                dag: "d".into(),
                edges: vec![TransitionEdge {
                    from: "pending".into(),
                    to: "sanctioned".into(),
                }],
            }),
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        assert!(r.is_clean());
    }

    #[test]
    fn escalation_rule_arg_not_declared_is_well_formedness_error() {
        let mut vc = bare_verb_config();
        vc.args = vec![arg("count")]; // declares `count` only
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Preserving,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Benign,
                escalation: vec![EscalationRule {
                    name: "phantom_arg".into(),
                    when: EscalationPredicate::ArgEq {
                        arg: "undeclared_arg".into(), // not in args list
                        value: json!(true),
                    },
                    tier: ConsequenceTier::Reviewable,
                    reason: None,
                }],
            },
            transitions: None,
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        assert_eq!(r.well_formedness.len(), 1);
        assert!(matches!(
            r.well_formedness[0],
            WellFormednessError::EscalationArgNotDeclared { .. }
        ));
    }

    #[test]
    fn escalation_tier_below_baseline_is_well_formedness_error() {
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Preserving,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::RequiresConfirmation,
                escalation: vec![EscalationRule {
                    name: "demote".into(),
                    when: EscalationPredicate::ContextFlag {
                        flag: "f".into(),
                    },
                    tier: ConsequenceTier::Benign, // strictly < baseline
                    reason: None,
                }],
            },
            transitions: None,
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        assert_eq!(r.well_formedness.len(), 1);
        assert!(matches!(
            r.well_formedness[0],
            WellFormednessError::EscalationTierBelowBaseline { .. }
        ));
    }

    #[test]
    fn escalation_arg_refs_through_boolean_combinators() {
        let mut vc = bare_verb_config();
        vc.args = vec![arg("known_arg")];
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Preserving,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Benign,
                escalation: vec![EscalationRule {
                    name: "compound".into(),
                    when: EscalationPredicate::And {
                        preds: vec![
                            EscalationPredicate::ArgEq {
                                arg: "known_arg".into(),
                                value: json!(1),
                            },
                            EscalationPredicate::Not {
                                pred: Box::new(EscalationPredicate::ArgGt {
                                    arg: "phantom".into(), // unknown
                                    value: 0.0,
                                }),
                            },
                        ],
                    },
                    tier: ConsequenceTier::Reviewable,
                    reason: None,
                }],
            },
            transitions: None,
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        assert_eq!(r.well_formedness.len(), 1);
        let WellFormednessError::EscalationArgNotDeclared { arg, .. } =
            &r.well_formedness[0]
        else {
            panic!("expected EscalationArgNotDeclared");
        };
        assert_eq!(arg, "phantom");
    }

    #[test]
    fn unknown_dag_reference_is_well_formedness_error_when_known_dags_given() {
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Transition,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Reviewable,
                escalation: vec![],
            },
            transitions: Some(VerbTransitions {
                dag: "typo_dag".into(),
                edges: vec![TransitionEdge {
                    from: "a".into(),
                    to: "b".into(),
                }],
            }),
        });
        let mut known = HashSet::new();
        known.insert("real_dag".to_string());
        let ctx = ValidationContext {
            known_dags: known,
            require_declaration: false,
        };
        let r = validate_verb("test.verb", &vc, &ctx);
        assert_eq!(r.well_formedness.len(), 1);
        assert!(matches!(
            r.well_formedness[0],
            WellFormednessError::UnknownDagReference { .. }
        ));
    }

    // =========================================================================
    // P.1.d — policy-sanity warnings + duplicate-rule-name well-formedness
    // =========================================================================

    #[test]
    fn unreachable_escalation_warning_when_rule_tier_equals_baseline() {
        // Narrow warning: a rule whose tier == baseline can never raise
        // effective_tier via max(). Mechanically dead, not a bug.
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Preserving,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Reviewable,
                escalation: vec![EscalationRule {
                    name: "redundant".into(),
                    when: EscalationPredicate::ContextFlag { flag: "f".into() },
                    tier: ConsequenceTier::Reviewable, // == baseline → dead
                    reason: None,
                }],
            },
            transitions: None,
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        // NOT a structural or well-formedness error — just a warning.
        assert!(r.structural.is_empty());
        assert!(r.well_formedness.is_empty());
        assert_eq!(r.warnings.len(), 1);
        assert!(matches!(
            r.warnings[0],
            PolicyWarning::UnreachableEscalation { .. }
        ));
    }

    #[test]
    fn escalation_tier_above_baseline_does_not_warn() {
        // Sanity: the reachable case is silent.
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Preserving,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Benign,
                escalation: vec![EscalationRule {
                    name: "real".into(),
                    when: EscalationPredicate::ContextFlag { flag: "f".into() },
                    tier: ConsequenceTier::Reviewable, // > baseline → real
                    reason: None,
                }],
            },
            transitions: None,
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        assert!(r.is_clean());
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn duplicate_rule_name_is_well_formedness_error() {
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Preserving,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Benign,
                escalation: vec![
                    EscalationRule {
                        name: "shared".into(),
                        when: EscalationPredicate::ContextFlag { flag: "a".into() },
                        tier: ConsequenceTier::Reviewable,
                        reason: None,
                    },
                    EscalationRule {
                        name: "shared".into(), // duplicate
                        when: EscalationPredicate::ContextFlag { flag: "b".into() },
                        tier: ConsequenceTier::RequiresConfirmation,
                        reason: None,
                    },
                ],
            },
            transitions: None,
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        // One error per duplicated name, not per occurrence.
        let dup_errors: Vec<_> = r
            .well_formedness
            .iter()
            .filter(|e| matches!(e, WellFormednessError::DuplicateRuleName { .. }))
            .collect();
        assert_eq!(dup_errors.len(), 1);
        let WellFormednessError::DuplicateRuleName {
            rule_name,
            occurrences,
            ..
        } = dup_errors[0]
        else {
            panic!();
        };
        assert_eq!(rule_name, "shared");
        assert_eq!(*occurrences, 2);
    }

    #[test]
    fn three_or_more_duplicates_reports_once() {
        let mk_rule = |name: &str, flag: &str, tier: ConsequenceTier| EscalationRule {
            name: name.into(),
            when: EscalationPredicate::ContextFlag { flag: flag.into() },
            tier,
            reason: None,
        };
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Preserving,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Benign,
                escalation: vec![
                    mk_rule("collision", "a", ConsequenceTier::Reviewable),
                    mk_rule("collision", "b", ConsequenceTier::RequiresConfirmation),
                    mk_rule("collision", "c", ConsequenceTier::RequiresExplicitAuthorisation),
                ],
            },
            transitions: None,
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        let dup_errors: Vec<_> = r
            .well_formedness
            .iter()
            .filter(|e| matches!(e, WellFormednessError::DuplicateRuleName { .. }))
            .collect();
        assert_eq!(dup_errors.len(), 1, "one error per name, not per occurrence");
        if let WellFormednessError::DuplicateRuleName { occurrences, .. } = dup_errors[0] {
            assert_eq!(*occurrences, 3);
        }
    }

    #[test]
    fn p10_silence_on_unusual_legitimate_combinations() {
        // v1.1 §6.2 enumerates legitimate unusual combinations that the
        // validator MUST NOT warn on. This test asserts the warning list
        // is empty for each of them, in addition to the error list.

        // (1) State-preserving + requires_explicit_authorisation (exports).
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Preserving,
            external_effects: vec![ExternalEffect::Emitting],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::RequiresExplicitAuthorisation,
                escalation: vec![],
            },
            transitions: None,
        });
        let r = validate_verb("export.verb", &vc, &ValidationContext::default());
        assert!(r.is_clean() && r.warnings.is_empty());

        // (2) State-transition + benign (cosmetic reordering).
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Transition,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Benign,
                escalation: vec![],
            },
            transitions: Some(VerbTransitions {
                dag: "any".into(),
                edges: vec![TransitionEdge {
                    from: "a".into(),
                    to: "b".into(),
                }],
            }),
        });
        let r = validate_verb("reorder.verb", &vc, &ValidationContext::default());
        assert!(r.is_clean() && r.warnings.is_empty());

        // (3) State-transition + external_effects: [] + requires_explicit_authorisation
        // (sanctions-state advance).
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Transition,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::RequiresExplicitAuthorisation,
                escalation: vec![],
            },
            transitions: Some(VerbTransitions {
                dag: "any".into(),
                edges: vec![TransitionEdge {
                    from: "pending".into(),
                    to: "sanctioned".into(),
                }],
            }),
        });
        let r = validate_verb("sanction.apply", &vc, &ValidationContext::default());
        assert!(r.is_clean() && r.warnings.is_empty());
    }

    #[test]
    fn unknown_dag_reference_skipped_when_known_dags_empty() {
        // P.1.c runs without the P.2 DAG taxonomy; the cross-check only
        // fires when the caller provides known-DAG info.
        let mut vc = bare_verb_config();
        vc.three_axis = Some(ThreeAxisDeclaration {
            state_effect: StateEffect::Transition,
            external_effects: vec![],
            consequence: ConsequenceDeclaration {
                baseline: ConsequenceTier::Reviewable,
                escalation: vec![],
            },
            transitions: Some(VerbTransitions {
                dag: "anything".into(),
                edges: vec![TransitionEdge {
                    from: "a".into(),
                    to: "b".into(),
                }],
            }),
        });
        let r = validate_verb("test.verb", &vc, &ValidationContext::default());
        assert!(r.is_clean());
    }

    // =========================================================================
    // V1.2-5 — pack-hygiene validation
    // =========================================================================

    #[test]
    fn pack_fqn_that_resolves_to_declared_verb_is_clean() {
        let declared: HashSet<String> = ["foo.bar", "baz.qux"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let macros: HashSet<String> = HashSet::new();
        let pack = vec![
            ("test-pack".to_string(), "foo.bar".to_string()),
            ("test-pack".to_string(), "baz.qux".to_string()),
        ];
        let errs = validate_pack_fqns(&declared, &macros, pack);
        assert!(errs.is_empty(), "all pack entries resolve → clean");
    }

    #[test]
    fn pack_fqn_that_resolves_to_macro_is_clean() {
        let declared: HashSet<String> = HashSet::new();
        let macros: HashSet<String> = ["instrument.setup-equity"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let pack = vec![(
            "instrument-matrix".to_string(),
            "instrument.setup-equity".to_string(),
        )];
        let errs = validate_pack_fqns(&declared, &macros, pack);
        assert!(errs.is_empty(), "macro-resolved pack entry is clean");
    }

    #[test]
    fn unresolved_pack_fqn_reports_error() {
        let declared: HashSet<String> =
            ["foo.bar"].iter().map(|s| (*s).to_string()).collect();
        let macros: HashSet<String> = HashSet::new();
        // `baz.qux` isn't in either declared or macro sets.
        let pack = vec![
            ("test-pack".to_string(), "foo.bar".to_string()),
            ("test-pack".to_string(), "baz.qux".to_string()),
        ];
        let errs = validate_pack_fqns(&declared, &macros, pack);
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            WellFormednessError::PackFqnWithoutDeclaration { pack_name, fqn } => {
                assert_eq!(pack_name, "test-pack");
                assert_eq!(fqn, "baz.qux");
            }
            _ => panic!("expected PackFqnWithoutDeclaration"),
        }
    }

    #[test]
    fn multiple_unresolved_fqns_report_one_per_entry() {
        // A-2's 11-FQN scenario (pruned in commit 17d6593b). If those
        // pack entries were still around, validator would surface one
        // error per entry.
        let declared: HashSet<String> = HashSet::new();
        let macros: HashSet<String> = HashSet::new();
        let pack = vec![
            ("instrument-matrix".to_string(), "matrix-overlay.apply".to_string()),
            ("instrument-matrix".to_string(), "matrix-overlay.diff".to_string()),
            ("instrument-matrix".to_string(), "delivery.create".to_string()),
        ];
        let errs = validate_pack_fqns(&declared, &macros, pack);
        assert_eq!(errs.len(), 3);
    }

    #[test]
    fn collect_declared_fqns_aggregates_across_domains() {
        let yaml = r#"
version: "1.0"
domains:
  foo:
    description: "test"
    verbs:
      bar:
        description: "test"
        behavior: crud
      baz:
        description: "test"
        behavior: crud
  qux:
    description: "test"
    verbs:
      wobble:
        description: "test"
        behavior: crud
"#;
        let cfg: VerbsConfig = serde_yaml::from_str(yaml).unwrap();
        let declared = collect_declared_fqns(&cfg);
        assert_eq!(declared.len(), 3);
        assert!(declared.contains("foo.bar"));
        assert!(declared.contains("foo.baz"));
        assert!(declared.contains("qux.wobble"));
    }
}
