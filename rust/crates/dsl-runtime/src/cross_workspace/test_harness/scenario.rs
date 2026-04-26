//! Scenario YAML schema for the cross-workspace DAG test harness.
//!
//! See `tests/fixtures/cross_workspace_dag/README.md` for the documented
//! shape and examples.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Execution mode for a scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScenarioMode {
    /// Mock providers only (in-memory, no DB). Fast (~ms per step).
    #[default]
    Mock,
    /// Live providers backed by ephemeral Postgres DB via `#[sqlx::test]`.
    /// Catches schema drift, real SQL errors, FK violations.
    Live,
    /// Both modes — runner exercises the scenario twice. Useful for
    /// fixtures intended to validate both paths.
    Both,
}

impl ScenarioMode {
    pub fn includes_mock(self) -> bool {
        matches!(self, Self::Mock | Self::Both)
    }
    pub fn includes_live(self) -> bool {
        matches!(self, Self::Live | Self::Both)
    }
}

/// Top-level scenario file. One file = one scenario; multi-step.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Scenario {
    pub name: String,
    pub suite_id: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Mock / Live / Both. Default Mock for backwards compat with the
    /// 6 fixtures shipped before live-mode existed.
    #[serde(default)]
    pub mode: ScenarioMode,
    /// Override DAG taxonomies dir (relative to repo root). Defaults to
    /// `rust/config/sem_os_seeds/dag_taxonomies/`.
    #[serde(default)]
    pub dag_taxonomies_dir: Option<String>,
    /// Symbolic-name → UUID alias map. UUIDs in the rest of the scenario
    /// are referenced by alias name; the runner resolves them.
    #[serde(default)]
    pub entity_aliases: HashMap<String, String>,
    /// (Live mode only) Extra rows the seed engine should insert before
    /// `initial_state`, to satisfy FK / NOT NULL parents that aren't
    /// themselves participants in any check_transition step.
    /// Each entry maps a column name → entity alias OR literal value.
    #[serde(default)]
    pub seed_dependencies: Vec<SeedDependency>,
    /// Initial in-memory state assigned to mock SlotStateProvider.
    /// (Live mode: same data, INSERTed into the live DB instead.)
    #[serde(default)]
    pub initial_state: Vec<StateEntry>,
    /// Mock predicate truth table. `predicate_string → list of (target, source)` mappings.
    /// (Ignored in live mode — real predicates execute SQL.)
    #[serde(default)]
    pub predicates: HashMap<String, Vec<PredicateEntry>>,
    /// Mock parent → children entries for cascade tests.
    /// Keyed by `"workspace.slot"` (parent slot identity).
    /// (Ignored in live mode — real ChildEntityResolver walks the DB.)
    #[serde(default)]
    pub children: HashMap<String, HashMap<String, Vec<ChildEntry>>>,
    pub steps: Vec<ScenarioStep>,
}

/// Seed-dependency row inserted by the live-mode seed engine before
/// `initial_state`. Use for FK-required parents that don't have a
/// state machine of their own (e.g. `entities` rows referenced by
/// `cases.cbu_id`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SeedDependency {
    /// Bare table name (no schema prefix; the engine adds "ob-poc").
    pub table: String,
    /// `column_name → value`. Values can be `"alias:foo"` to reference
    /// an entity_alias UUID, or literal strings/numbers.
    pub columns: HashMap<String, serde_json::Value>,
}

/// One initial-state row.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateEntry {
    pub workspace: String,
    pub slot: String,
    /// Alias name of the entity (resolved to UUID via `entity_aliases`).
    pub entity: String,
    /// Initial state. `null` means "row exists but state column is NULL".
    #[serde(default)]
    pub state: Option<String>,
    /// (Live mode only) Additional column overrides for this row's INSERT.
    /// Values can be `"alias:<name>"` to inline-resolve an entity alias, or
    /// literal strings/numbers/null. Use this to set FK-bridging columns
    /// the predicate joins look up (e.g. `cases.client_group_id` matching
    /// `deals.primary_client_group_id`). Mock mode ignores this field.
    #[serde(default)]
    pub attrs: serde_json::Map<String, serde_json::Value>,
}

/// One mock predicate truth-table row: maps a target_id to a source_id.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PredicateEntry {
    pub target: String,
    pub source: String,
}

/// One child-resolver entry: child slot + child entity that belongs to a
/// parent identified by the outer key.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChildEntry {
    pub workspace: String,
    pub slot: String,
    pub entity: String,
}

/// A scenario step. Exactly one of the operation fields is set.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScenarioStep {
    /// Optional human-readable label for the step.
    #[serde(default)]
    pub name: Option<String>,
    /// Mode A — fire GateChecker.
    #[serde(default)]
    pub check_transition: Option<CheckTransitionOp>,
    /// Mode B — fire DerivedStateEvaluator.
    #[serde(default)]
    pub evaluate_derived: Option<EvaluateDerivedOp>,
    /// Mode C — fire CascadePlanner.
    #[serde(default)]
    pub plan_cascade: Option<PlanCascadeOp>,
    /// Test scaffolding: directly set state on the mock provider.
    #[serde(default)]
    pub mutate: Option<Vec<StateEntry>>,
    /// Assertions for this step.
    #[serde(default)]
    pub expect: Option<StepExpectation>,
}

/// Mode A operation: check a transition for cross-workspace constraint
/// violations.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CheckTransitionOp {
    pub workspace: String,
    pub slot: String,
    pub entity: String,
    pub from: String,
    pub to: String,
}

/// Mode B operation: evaluate a named derived state on a host entity.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EvaluateDerivedOp {
    pub workspace: String,
    pub slot: String,
    /// derived_id from the DAG declaration.
    pub derived_id: String,
    /// Host entity (the row whose aggregate is computed).
    pub host_entity: String,
}

/// Mode C operation: plan cascade actions for a parent transition.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanCascadeOp {
    pub parent_workspace: String,
    pub parent_slot: String,
    pub parent_entity: String,
    pub parent_new_state: String,
}

/// Per-step assertions. Only specified fields are checked.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StepExpectation {
    /// Mode A: expected gate violations (order-insensitive).
    /// Empty list explicitly asserts "no violations".
    #[serde(default)]
    pub violations: Option<Vec<ExpectedViolation>>,
    /// Mode B: expected DerivedStateValue.
    #[serde(default)]
    pub derived: Option<ExpectedDerivedValue>,
    /// Mode C: expected cascade actions (order-insensitive).
    #[serde(default)]
    pub cascade_actions: Option<Vec<ExpectedCascadeAction>>,
}

/// One expected gate violation. Optional fields only checked when present.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExpectedViolation {
    pub constraint_id: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub required_state: Option<Vec<String>>,
    /// `null` matches the case where the source row exists with NULL state
    /// (not the case where the predicate didn't resolve).
    #[serde(default)]
    pub actual_state: Option<Option<String>>,
}

/// Expected DerivedStateValue.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExpectedDerivedValue {
    pub satisfied: bool,
    /// Optional per-condition expectations. Compared by index.
    #[serde(default)]
    pub conditions: Option<Vec<ExpectedCondition>>,
}

/// One expected condition result inside a DerivedStateValue.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExpectedCondition {
    pub satisfied: bool,
    /// Substring match against ConditionResult.description (optional).
    #[serde(default)]
    pub description_contains: Option<String>,
}

/// One expected cascade action.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExpectedCascadeAction {
    pub child_workspace: String,
    pub child_slot: String,
    pub child_entity: String,
    pub target_state: String,
    #[serde(default)]
    pub severity: Option<String>,
}
