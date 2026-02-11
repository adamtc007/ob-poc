//! Precondition Filter — DAG-aware eligibility gating for verbs (Phase PD).
//!
//! Parses `precondition_checks` from verb YAML lifecycle config and evaluates
//! them against the current `ContextStack` to determine which verbs are
//! eligible for execution.
//!
//! # Format
//!
//! Precondition strings use the `"key:value"` format:
//! - `"requires_scope:cbu"` — a CBU must be in scope
//! - `"requires_scope:client_group"` — a client group must be set
//! - `"requires_prior:cbu.create"` — verb `cbu.create` must have been executed
//! - `"requires_entities:entity"` — at least one entity must be in scope
//! - `"forbids_prior:cbu.delete"` — verb `cbu.delete` must NOT have been executed
//!
//! # Future Enhancement
//!
//! The addendum specifies richer types (`PriorVerbRequirement` with `scope`,
//! `min_count`, `with_args` fields). The current `"key:value"` string format
//! is pragmatic for the existing verb YAML — a richer schema can be introduced
//! when verb authors need it, without changing the evaluation logic.

use std::collections::HashSet;

use super::context_stack::ContextStack;
use super::verb_config_index::VerbConfigIndex;

// ============================================================================
// Types
// ============================================================================

/// Parsed preconditions for a single verb.
#[derive(Debug, Clone, Default)]
pub struct Preconditions {
    /// Scope requirements (e.g., `["cbu"]` means a CBU must be in scope).
    pub requires_scope: Vec<String>,
    /// Prior verb execution requirements (e.g., `["cbu.create"]`).
    pub requires_prior: Vec<String>,
    /// Entity requirements (e.g., `["entity"]` means entities must be loaded).
    pub requires_entities: Vec<String>,
    /// Forbidden prior verbs (e.g., `["cbu.delete"]` blocks if executed).
    pub forbids_prior: Vec<String>,
}

/// Eligibility mode controls which runbook entries count as "facts".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EligibilityMode {
    /// Only executed entries count as facts.
    Executable,
    /// Executed + staged (confirmed but not yet executed) entries count as facts.
    Plan,
}

/// Result of a precondition check for a single verb.
#[derive(Debug, Clone)]
pub struct PreconditionResult {
    /// Fully qualified verb name.
    pub verb_fqn: String,
    /// Whether all preconditions are met.
    pub met: bool,
    /// If not met, why — for "why not" suggestions.
    pub unmet_reasons: Vec<UnmetReason>,
}

/// Reason a precondition was not met.
#[derive(Debug, Clone)]
pub struct UnmetReason {
    /// The precondition that failed (e.g., `"requires_scope:cbu"`).
    pub precondition: String,
    /// Human-readable explanation.
    pub explanation: String,
    /// Suggested verb to satisfy the precondition (if known).
    pub suggested_verb: Option<String>,
}

/// Summary statistics for precondition filtering.
#[derive(Debug, Clone, Default)]
pub struct FilterStats {
    /// Candidates before filtering.
    pub before_count: usize,
    /// Candidates after filtering.
    pub after_count: usize,
    /// Verbs removed with their reasons.
    pub removed: Vec<PreconditionResult>,
}

// ============================================================================
// Parsing
// ============================================================================

/// Parse precondition strings from YAML into a structured `Preconditions`.
///
/// Format: `"key:value"` where key ∈ {requires_scope, requires_prior,
/// requires_entities, forbids_prior}.
pub fn parse_preconditions(checks: &[String]) -> Preconditions {
    let mut result = Preconditions::default();

    for check in checks {
        if let Some((key, value)) = check.split_once(':') {
            let value = value.trim().to_string();
            match key.trim() {
                "requires_scope" => result.requires_scope.push(value),
                "requires_prior" => result.requires_prior.push(value),
                "requires_entities" => result.requires_entities.push(value),
                "forbids_prior" => result.forbids_prior.push(value),
                other => {
                    tracing::warn!("Unknown precondition key: '{}' in '{}'", other, check);
                }
            }
        } else {
            tracing::warn!("Malformed precondition (expected 'key:value'): '{}'", check);
        }
    }

    result
}

// ============================================================================
// Evaluation
// ============================================================================

/// Check whether a verb's preconditions are met given the current context.
///
/// Pure function — no side effects.
pub fn preconditions_met(
    preconditions: &Preconditions,
    context: &ContextStack,
    mode: EligibilityMode,
) -> PreconditionResult {
    let mut unmet = Vec::new();

    // --- requires_scope ---
    for scope_req in &preconditions.requires_scope {
        let met = match scope_req.as_str() {
            "cbu" => {
                context.derived_scope.default_cbu.is_some()
                    || !context.derived_scope.loaded_cbu_ids.is_empty()
            }
            "client_group" => context.derived_scope.client_group_id.is_some(),
            "book" => context.derived_scope.default_book.is_some(),
            other => {
                tracing::debug!("Unknown scope requirement: '{}'", other);
                false
            }
        };
        if !met {
            unmet.push(UnmetReason {
                precondition: format!("requires_scope:{}", scope_req),
                explanation: format!("No {} in scope", scope_req),
                suggested_verb: suggest_verb_for_scope(scope_req),
            });
        }
    }

    // --- requires_prior ---
    let executed_verbs = executed_verb_set(context, mode);
    for prior_req in &preconditions.requires_prior {
        if !executed_verbs.contains(prior_req.as_str()) {
            unmet.push(UnmetReason {
                precondition: format!("requires_prior:{}", prior_req),
                explanation: format!("Verb '{}' has not been executed yet", prior_req),
                suggested_verb: Some(prior_req.clone()),
            });
        }
    }

    // --- requires_entities ---
    for entity_req in &preconditions.requires_entities {
        let met = match entity_req.as_str() {
            "entity" => !context.derived_scope.loaded_cbu_ids.is_empty(),
            _ => {
                tracing::debug!("Unknown entity requirement: '{}'", entity_req);
                false
            }
        };
        if !met {
            unmet.push(UnmetReason {
                precondition: format!("requires_entities:{}", entity_req),
                explanation: format!("No {} entities in scope", entity_req),
                suggested_verb: None,
            });
        }
    }

    // --- forbids_prior ---
    for forbid_req in &preconditions.forbids_prior {
        if executed_verbs.contains(forbid_req.as_str()) {
            unmet.push(UnmetReason {
                precondition: format!("forbids_prior:{}", forbid_req),
                explanation: format!(
                    "Verb '{}' has already been executed (forbidden)",
                    forbid_req
                ),
                suggested_verb: None,
            });
        }
    }

    let met = unmet.is_empty();
    PreconditionResult {
        verb_fqn: String::new(), // Caller fills this
        met,
        unmet_reasons: unmet,
    }
}

/// Batch-check preconditions for all verbs in the index.
///
/// Returns the set of verb FQNs whose preconditions are met.
pub fn verbs_with_met_preconditions(
    verb_index: &VerbConfigIndex,
    context: &ContextStack,
    mode: EligibilityMode,
) -> HashSet<String> {
    let mut eligible = HashSet::new();

    for (fqn, entry) in verb_index.iter() {
        if entry.precondition_checks.is_empty() {
            // No preconditions → always eligible
            eligible.insert(fqn.clone());
            continue;
        }

        let parsed = parse_preconditions(&entry.precondition_checks);
        let result = preconditions_met(&parsed, context, mode);
        if result.met {
            eligible.insert(fqn.clone());
        }
    }

    eligible
}

/// Filter verb candidates by preconditions, returning stats for the DecisionLog.
///
/// Candidates whose preconditions are not met are removed from the list.
/// If the best pre-filter candidate was removed, the first unmet reason's
/// `suggested_verb` is returned as a "why not" hint.
pub fn filter_by_preconditions(
    candidates: &mut Vec<super::types::VerbCandidate>,
    verb_index: &VerbConfigIndex,
    context: &ContextStack,
    mode: EligibilityMode,
) -> FilterStats {
    let before_count = candidates.len();
    let eligible = verbs_with_met_preconditions(verb_index, context, mode);

    let mut removed = Vec::new();

    candidates.retain(|c| {
        if eligible.contains(&c.verb_fqn) {
            true
        } else {
            // Build detailed result for "why not" reporting
            let checks = verb_index
                .get(&c.verb_fqn)
                .map(|e| e.precondition_checks.as_slice())
                .unwrap_or(&[]);
            let parsed = parse_preconditions(checks);
            let mut result = preconditions_met(&parsed, context, mode);
            result.verb_fqn = c.verb_fqn.clone();
            removed.push(result);
            false
        }
    });

    FilterStats {
        before_count,
        after_count: candidates.len(),
        removed,
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Collect the set of verb FQNs that count as "facts" given the mode.
///
/// In `Executable` mode, only completed verbs count.
/// In `Plan` mode, completed + staged verbs count.
fn executed_verb_set(context: &ContextStack, mode: EligibilityMode) -> HashSet<&str> {
    let mut verbs: HashSet<&str> = context.executed_verbs.iter().map(|s| s.as_str()).collect();

    if mode == EligibilityMode::Plan {
        for v in &context.staged_verbs {
            verbs.insert(v.as_str());
        }
    }

    verbs
}

/// Suggest a verb that would satisfy a scope requirement.
fn suggest_verb_for_scope(scope_req: &str) -> Option<String> {
    match scope_req {
        "cbu" => Some("session.load-cbu".to_string()),
        "client_group" => Some("session.load-cluster".to_string()),
        "book" => Some("session.load-galaxy".to_string()),
        _ => None,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repl::context_stack::{
        ContextStack, DerivedScope, ExclusionSet, FocusContext, OutcomeRegistry, RecentContext,
    };
    use std::collections::HashMap;
    use uuid::Uuid;

    fn empty_context() -> ContextStack {
        ContextStack {
            derived_scope: DerivedScope::default(),
            pack_staged: None,
            pack_executed: None,
            template_hint: None,
            focus: FocusContext::default(),
            recent: RecentContext::default(),
            exclusions: ExclusionSet::default(),
            outcomes: OutcomeRegistry::default(),
            accumulated_answers: HashMap::new(),
            executed_verbs: HashSet::new(),
            staged_verbs: HashSet::new(),
            turn: 0,
        }
    }

    fn context_with_cbu() -> ContextStack {
        let mut ctx = empty_context();
        ctx.derived_scope.default_cbu = Some(Uuid::new_v4());
        ctx.derived_scope.loaded_cbu_ids = vec![Uuid::new_v4()];
        ctx
    }

    fn context_with_client_group() -> ContextStack {
        let mut ctx = empty_context();
        ctx.derived_scope.client_group_id = Some(Uuid::new_v4());
        ctx.derived_scope.client_group_name = Some("Allianz".to_string());
        ctx
    }

    fn context_with_prior_verb(verb: &str) -> ContextStack {
        let mut ctx = context_with_cbu();
        ctx.executed_verbs.insert(verb.to_string());
        ctx
    }

    // --- Parsing ---

    #[test]
    fn test_parse_empty() {
        let p = parse_preconditions(&[]);
        assert!(p.requires_scope.is_empty());
        assert!(p.requires_prior.is_empty());
        assert!(p.requires_entities.is_empty());
        assert!(p.forbids_prior.is_empty());
    }

    #[test]
    fn test_parse_requires_scope() {
        let p = parse_preconditions(&["requires_scope:cbu".to_string()]);
        assert_eq!(p.requires_scope, vec!["cbu"]);
    }

    #[test]
    fn test_parse_requires_prior() {
        let p = parse_preconditions(&["requires_prior:cbu.create".to_string()]);
        assert_eq!(p.requires_prior, vec!["cbu.create"]);
    }

    #[test]
    fn test_parse_forbids_prior() {
        let p = parse_preconditions(&["forbids_prior:cbu.delete".to_string()]);
        assert_eq!(p.forbids_prior, vec!["cbu.delete"]);
    }

    #[test]
    fn test_parse_multiple() {
        let p = parse_preconditions(&[
            "requires_scope:cbu".to_string(),
            "requires_prior:cbu.create".to_string(),
            "forbids_prior:cbu.delete".to_string(),
        ]);
        assert_eq!(p.requires_scope, vec!["cbu"]);
        assert_eq!(p.requires_prior, vec!["cbu.create"]);
        assert_eq!(p.forbids_prior, vec!["cbu.delete"]);
    }

    #[test]
    fn test_parse_malformed_ignored() {
        let p = parse_preconditions(&["no_colon_here".to_string()]);
        assert!(p.requires_scope.is_empty());
    }

    // --- requires_scope ---

    #[test]
    fn test_requires_scope_cbu_met() {
        let p = parse_preconditions(&["requires_scope:cbu".to_string()]);
        let result = preconditions_met(&p, &context_with_cbu(), EligibilityMode::Executable);
        assert!(result.met);
        assert!(result.unmet_reasons.is_empty());
    }

    #[test]
    fn test_requires_scope_cbu_unmet() {
        let p = parse_preconditions(&["requires_scope:cbu".to_string()]);
        let result = preconditions_met(&p, &empty_context(), EligibilityMode::Executable);
        assert!(!result.met);
        assert_eq!(result.unmet_reasons.len(), 1);
        assert_eq!(
            result.unmet_reasons[0].suggested_verb,
            Some("session.load-cbu".to_string())
        );
    }

    #[test]
    fn test_requires_scope_client_group_met() {
        let p = parse_preconditions(&["requires_scope:client_group".to_string()]);
        let result = preconditions_met(
            &p,
            &context_with_client_group(),
            EligibilityMode::Executable,
        );
        assert!(result.met);
    }

    #[test]
    fn test_requires_scope_client_group_unmet() {
        let p = parse_preconditions(&["requires_scope:client_group".to_string()]);
        let result = preconditions_met(&p, &empty_context(), EligibilityMode::Executable);
        assert!(!result.met);
    }

    // --- requires_prior ---

    #[test]
    fn test_requires_prior_met() {
        let p = parse_preconditions(&["requires_prior:cbu.create".to_string()]);
        let ctx = context_with_prior_verb("cbu.create");
        let result = preconditions_met(&p, &ctx, EligibilityMode::Executable);
        assert!(result.met);
    }

    #[test]
    fn test_requires_prior_unmet() {
        let p = parse_preconditions(&["requires_prior:cbu.create".to_string()]);
        let result = preconditions_met(&p, &empty_context(), EligibilityMode::Executable);
        assert!(!result.met);
        assert_eq!(
            result.unmet_reasons[0].suggested_verb,
            Some("cbu.create".to_string())
        );
    }

    // --- forbids_prior ---

    #[test]
    fn test_forbids_prior_not_executed() {
        let p = parse_preconditions(&["forbids_prior:cbu.delete".to_string()]);
        let result = preconditions_met(&p, &empty_context(), EligibilityMode::Executable);
        assert!(result.met); // Not executed → not forbidden
    }

    #[test]
    fn test_forbids_prior_executed() {
        let p = parse_preconditions(&["forbids_prior:cbu.delete".to_string()]);
        let ctx = context_with_prior_verb("cbu.delete");
        let result = preconditions_met(&p, &ctx, EligibilityMode::Executable);
        assert!(!result.met);
    }

    // --- EligibilityMode ---

    #[test]
    fn test_plan_mode_includes_staged() {
        let p = parse_preconditions(&["requires_prior:cbu.create".to_string()]);
        let mut ctx = empty_context();
        // Add as staged verb, NOT as executed
        ctx.staged_verbs.insert("cbu.create".to_string());

        // Executable mode: should NOT see staged
        let result = preconditions_met(&p, &ctx, EligibilityMode::Executable);
        assert!(!result.met);

        // Plan mode: should see staged
        let result = preconditions_met(&p, &ctx, EligibilityMode::Plan);
        assert!(result.met);
    }

    // --- Empty preconditions ---

    #[test]
    fn test_empty_preconditions_always_met() {
        let p = Preconditions::default();
        let result = preconditions_met(&p, &empty_context(), EligibilityMode::Executable);
        assert!(result.met);
    }

    // --- "Why not" suggestions ---

    #[test]
    fn test_why_not_suggestion_scope() {
        let p = parse_preconditions(&["requires_scope:cbu".to_string()]);
        let result = preconditions_met(&p, &empty_context(), EligibilityMode::Executable);
        assert!(!result.met);
        let reason = &result.unmet_reasons[0];
        assert!(reason.explanation.contains("No cbu in scope"));
        assert_eq!(reason.suggested_verb, Some("session.load-cbu".to_string()));
    }

    #[test]
    fn test_why_not_suggestion_prior() {
        let p = parse_preconditions(&["requires_prior:kyc.create-case".to_string()]);
        let result = preconditions_met(&p, &empty_context(), EligibilityMode::Executable);
        assert!(!result.met);
        let reason = &result.unmet_reasons[0];
        assert_eq!(reason.suggested_verb, Some("kyc.create-case".to_string()));
    }
}
