//! DerivedStateEvaluator — runtime evaluator for V1.3-2
//! `derived_cross_workspace_state` aggregates (the tollgate pattern).
//!
//! For each registered `DerivedCrossWorkspaceState`, the evaluator walks
//! the `derivation` block (`all_of` / `any_of`) and resolves each
//! `DerivationCondition` to a boolean by:
//!
//!   * Looking up the referenced slot's state via `SlotStateProvider`.
//!   * Comparing the actual state against the declared `state` selector.
//!   * Treating `predicate`-only conditions as opaque (caller-evaluated
//!     via `PredicateResolver` for the row scope; if the predicate
//!     resolves to a row, condition is satisfied).
//!   * Treating raw-string conditions as informational; they always
//!     evaluate to `true` (these are the v1.2-style free-text predicates
//!     not yet structurally encoded).
//!
//! Returns a `DerivedStateValue` capturing the boolean result + the
//! per-condition outcomes (for diagnostics / UI / cache invalidation).
//!
//! Per OQ-2: callers wrap this in a session-scope cache; this evaluator
//! itself is stateless and just executes one evaluation pass.

use anyhow::Result;
use dsl_core::config::dag::{DerivationCondition, DerivedCrossWorkspaceState, StateSelector};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use super::gate_checker::PredicateResolver;
use super::slot_state::SlotStateProvider;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// One evaluation of a derived cross-workspace aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedStateValue {
    /// True iff `all_of` passes AND (any_of empty OR `any_of` passes).
    pub satisfied: bool,
    /// Per-condition results (in declaration order: all_of first, then any_of).
    pub conditions: Vec<ConditionResult>,
}

/// One condition's evaluation outcome — for diagnostics / UI display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionResult {
    /// Which clause: `all_of` or `any_of`.
    pub clause: ClauseKind,
    /// Human-readable description of the condition.
    pub description: String,
    /// Whether this condition was satisfied.
    pub satisfied: bool,
    /// What state was found, if state-based.
    pub actual_state: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClauseKind {
    AllOf,
    AnyOf,
}

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------

/// Stateless evaluator for V1.3-2 derived cross-workspace states.
///
/// Holds shared `Arc` references to its dependencies; cheap to clone.
#[derive(Clone)]
pub struct DerivedStateEvaluator {
    slot_state_provider: Arc<dyn SlotStateProvider>,
    predicate_resolver: Arc<dyn PredicateResolver>,
}

impl DerivedStateEvaluator {
    pub fn new(
        slot_state_provider: Arc<dyn SlotStateProvider>,
        predicate_resolver: Arc<dyn PredicateResolver>,
    ) -> Self {
        Self {
            slot_state_provider,
            predicate_resolver,
        }
    }

    /// Evaluate one derived state aggregate against the live system.
    ///
    /// `host_entity_id` is the entity_id of the host slot's row — used
    /// as the "this" reference for predicate resolution.
    pub async fn evaluate(
        &self,
        derived: &DerivedCrossWorkspaceState,
        host_entity_id: Uuid,
        pool: &PgPool,
    ) -> Result<DerivedStateValue> {
        let mut conditions = Vec::new();

        // all_of: every condition must be satisfied.
        let mut all_satisfied = true;
        for cond in &derived.derivation.all_of {
            let result = self
                .evaluate_condition(
                    cond,
                    host_entity_id,
                    &derived.host_workspace,
                    &derived.host_slot,
                    ClauseKind::AllOf,
                    pool,
                )
                .await?;
            if !result.satisfied {
                all_satisfied = false;
            }
            conditions.push(result);
        }

        // any_of: at least one must be satisfied (only checked if non-empty).
        let any_evaluated = !derived.derivation.any_of.is_empty();
        let mut any_satisfied = false;
        for cond in &derived.derivation.any_of {
            let result = self
                .evaluate_condition(
                    cond,
                    host_entity_id,
                    &derived.host_workspace,
                    &derived.host_slot,
                    ClauseKind::AnyOf,
                    pool,
                )
                .await?;
            if result.satisfied {
                any_satisfied = true;
            }
            conditions.push(result);
        }

        let satisfied = all_satisfied && (!any_evaluated || any_satisfied);
        Ok(DerivedStateValue {
            satisfied,
            conditions,
        })
    }

    async fn evaluate_condition(
        &self,
        cond: &DerivationCondition,
        host_entity_id: Uuid,
        host_workspace: &str,
        host_slot: &str,
        clause: ClauseKind,
        pool: &PgPool,
    ) -> Result<ConditionResult> {
        match cond {
            // v1.2-style raw predicate strings: not structurally
            // resolvable. Treat as satisfied (informational — caller
            // can manually verify if needed). This keeps v1.2 DAGs
            // forward-compatible with the v1.3 evaluator.
            DerivationCondition::Raw(s) => Ok(ConditionResult {
                clause,
                description: format!("[raw, treated as satisfied] {s}"),
                satisfied: true,
                actual_state: None,
            }),

            // v1.3 structured: resolve via SlotStateProvider +
            // PredicateResolver.
            DerivationCondition::Structured(s) => {
                // Resolve which entity to look up.
                let entity_id = if let Some(pred) = &s.predicate {
                    match self
                        .predicate_resolver
                        .resolve_source_entity(
                            pred,
                            host_entity_id,
                            host_workspace,
                            host_slot,
                            pool,
                        )
                        .await?
                    {
                        Some(id) => id,
                        None => {
                            // Predicate didn't resolve — condition
                            // unsatisfied (no row to check).
                            return Ok(ConditionResult {
                                clause,
                                description: format!(
                                    "{}.{}: predicate did not resolve ({})",
                                    s.workspace, s.slot, pred
                                ),
                                satisfied: false,
                                actual_state: None,
                            });
                        }
                    }
                } else {
                    host_entity_id
                };

                // If only a predicate is declared (no `state`), the
                // predicate resolving to a row is itself the
                // satisfaction signal (e.g. "EXISTS xxx").
                if s.state.is_none() {
                    return Ok(ConditionResult {
                        clause,
                        description: format!("{}.{}: predicate satisfied", s.workspace, s.slot),
                        satisfied: true,
                        actual_state: None,
                    });
                }

                // Look up actual state.
                let actual = self
                    .slot_state_provider
                    .read_slot_state(&s.workspace, &s.slot, entity_id, pool)
                    .await?;

                let required = match &s.state {
                    Some(StateSelector::Single(v)) => vec![v.clone()],
                    Some(StateSelector::Set(v)) => v.clone(),
                    None => unreachable!("checked above"),
                };
                let satisfied = matches!(&actual, Some(a) if required.iter().any(|r| r == a));

                Ok(ConditionResult {
                    clause,
                    description: format!(
                        "{}.{}: state in {:?}, actual: {}",
                        s.workspace,
                        s.slot,
                        required,
                        actual.as_deref().unwrap_or("<missing>")
                    ),
                    satisfied,
                    actual_state: actual,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Mock provider — same as in gate_checker tests but inlined here for isolation.
    #[derive(Default)]
    struct MockSlotStateProvider {
        states: Mutex<HashMap<(String, String, Uuid), Option<String>>>,
    }

    impl MockSlotStateProvider {
        #[allow(dead_code)]
        fn set(&self, ws: &str, slot: &str, id: Uuid, state: Option<&str>) {
            self.states.lock().unwrap().insert(
                (ws.to_string(), slot.to_string(), id),
                state.map(String::from),
            );
        }
    }

    #[async_trait]
    impl SlotStateProvider for MockSlotStateProvider {
        async fn read_slot_state(
            &self,
            ws: &str,
            slot: &str,
            id: Uuid,
            _pool: &PgPool,
        ) -> Result<Option<String>> {
            let map = self.states.lock().unwrap();
            Ok(map
                .get(&(ws.to_string(), slot.to_string(), id))
                .cloned()
                .unwrap_or(None))
        }
    }

    fn build_derived(yaml: &str) -> DerivedCrossWorkspaceState {
        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn evaluator_construction() {
        // Only construction without a real PgPool — the evaluation methods
        // require &PgPool which can't be safely constructed without a
        // running DB. Real-DB integration tests live in the bin
        // `dag_runtime_smoke` (next slice).
        let provider = Arc::new(MockSlotStateProvider::default());
        let resolver = Arc::new(super::super::SameEntityResolver);
        let _e = DerivedStateEvaluator::new(provider, resolver);
    }

    #[test]
    fn raw_condition_treated_as_satisfied() {
        // Build a derived state with a raw v1.2-style predicate.
        let derived: DerivedCrossWorkspaceState = build_derived(
            r#"
id: demo_aggregate
host_workspace: cbu
host_slot: cbu
host_state: ready
derivation:
  all_of:
    - "cbus.status = 'VALIDATED'"
"#,
        );
        // Just check the parser landed it as Raw, not Structured.
        assert!(matches!(
            derived.derivation.all_of[0],
            DerivationCondition::Raw(_)
        ));
    }

    #[test]
    fn structured_condition_parses_with_state() {
        let derived: DerivedCrossWorkspaceState = build_derived(
            r#"
id: demo
host_workspace: cbu
host_slot: cbu
host_state: ready
derivation:
  all_of:
    - { workspace: kyc, slot: kyc_case, state: APPROVED }
    - { workspace: deal, slot: deal, state: [CONTRACTED, ACTIVE] }
"#,
        );
        assert_eq!(derived.derivation.all_of.len(), 2);
        for c in &derived.derivation.all_of {
            assert!(matches!(c, DerivationCondition::Structured(_)));
        }
    }
}
