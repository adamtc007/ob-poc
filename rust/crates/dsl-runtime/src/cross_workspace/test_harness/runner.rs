//! Scenario runner — loads YAML, wires mocks, executes steps, captures outcomes.

use anyhow::{anyhow, bail, Context, Result};
use dsl_core::config::DagRegistry;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use crate::cross_workspace::derived_state::{DerivedStateEvaluator, DerivedStateValue};
use crate::cross_workspace::gate_checker::{GateChecker, GateViolation};
use crate::cross_workspace::hierarchy_cascade::{CascadeAction, CascadePlanner};

use super::assertions::{
    check_cascade_actions, check_derived_value, check_violations, AssertionFailure,
};
use super::mocks::{MockChildEntityResolver, MockPredicateResolver, MockSlotStateProvider};
use super::scenario::{
    CheckTransitionOp, EvaluateDerivedOp, PlanCascadeOp, Scenario, ScenarioStep, StateEntry,
};

/// Default DAG taxonomies dir relative to repo root.
const DEFAULT_DAG_DIR: &str = "rust/config/sem_os_seeds/dag_taxonomies";

/// Loaded scenario + ready-to-fire runtime mocks.
pub struct ScenarioRunner {
    scenario: Scenario,
    registry: Arc<DagRegistry>,
    slot_state: Arc<MockSlotStateProvider>,
    predicate: Arc<MockPredicateResolver>,
    children: Arc<MockChildEntityResolver>,
    pool: PgPool,
    aliases: HashMap<String, Uuid>,
}

/// Outcome of running a single scenario.
#[derive(Debug)]
pub struct ScenarioReport {
    pub scenario_name: String,
    pub suite_id: String,
    pub steps_total: usize,
    pub steps_passed: usize,
    pub step_results: Vec<StepResult>,
}

impl ScenarioReport {
    pub fn passed(&self) -> bool {
        self.steps_passed == self.steps_total
    }

    pub fn failure_summary(&self) -> String {
        let mut out = format!(
            "Scenario '{}' ({}): {}/{} steps passed\n",
            self.scenario_name, self.suite_id, self.steps_passed, self.steps_total
        );
        for sr in &self.step_results {
            if !sr.failures.is_empty() {
                out.push_str(&format!(
                    "  Step {} ({}):\n",
                    sr.step_index,
                    sr.step_name.as_deref().unwrap_or("<unnamed>")
                ));
                for f in &sr.failures {
                    out.push_str(&format!("    - {}\n", f));
                }
            }
        }
        out
    }
}

#[derive(Debug)]
pub struct StepResult {
    pub step_index: usize,
    pub step_name: Option<String>,
    pub failures: Vec<AssertionFailure>,
}

impl ScenarioRunner {
    /// Load a scenario YAML file and run it end-to-end.
    pub async fn run_scenario_file(path: impl AsRef<Path>) -> Result<ScenarioReport> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading scenario file {}", path.display()))?;
        let scenario: Scenario = serde_yaml::from_str(&text)
            .with_context(|| format!("parsing scenario file {}", path.display()))?;
        Self::run_scenario(scenario).await
    }

    /// Run a pre-parsed scenario.
    pub async fn run_scenario(scenario: Scenario) -> Result<ScenarioReport> {
        let runner = Self::build(scenario)?;
        runner.execute().await
    }

    fn build(scenario: Scenario) -> Result<Self> {
        // 1. Resolve entity aliases (alias name → UUID).
        let mut aliases = HashMap::new();
        for (name, uuid_str) in &scenario.entity_aliases {
            let id = Uuid::parse_str(uuid_str)
                .with_context(|| format!("alias '{}' = '{}' is not a valid UUID", name, uuid_str))?;
            aliases.insert(name.clone(), id);
        }

        // 2. Load the real DagRegistry from YAML.
        let dag_dir = scenario
            .dag_taxonomies_dir
            .clone()
            .unwrap_or_else(|| DEFAULT_DAG_DIR.to_string());
        let dag_path = repo_root_join(&dag_dir);
        let registry = Arc::new(
            DagRegistry::from_dir(&dag_path)
                .with_context(|| format!("loading DAG taxonomies from {}", dag_path.display()))?,
        );

        // 3. Build the mock providers.
        let slot_state = Arc::new(MockSlotStateProvider::new());
        let predicate = Arc::new(MockPredicateResolver::new());
        let children = Arc::new(MockChildEntityResolver::new());

        // 4. Apply initial state.
        for entry in &scenario.initial_state {
            let id = lookup_alias(&aliases, &entry.entity)?;
            slot_state.set(&entry.workspace, &entry.slot, id, entry.state.as_deref());
        }

        // 5. Apply predicate truth tables.
        for (predicate_str, entries) in &scenario.predicates {
            for e in entries {
                let target_id = lookup_alias(&aliases, &e.target)?;
                let source_id = lookup_alias(&aliases, &e.source)?;
                predicate.set(predicate_str, target_id, source_id);
            }
        }

        // 6. Apply children mappings.
        for (parent_key, parent_map) in &scenario.children {
            let (parent_workspace, parent_slot) = parent_key
                .split_once('.')
                .ok_or_else(|| anyhow!("children key '{}' must be 'workspace.slot'", parent_key))?;
            for (parent_alias, child_entries) in parent_map {
                let parent_id = lookup_alias(&aliases, parent_alias)?;
                for ce in child_entries {
                    let child_id = lookup_alias(&aliases, &ce.entity)?;
                    children.add_child(
                        parent_workspace,
                        parent_slot,
                        parent_id,
                        &ce.workspace,
                        &ce.slot,
                        child_id,
                    );
                }
            }
        }

        // 7. Build the lazy pool.
        let pool = build_lazy_pool();

        Ok(Self {
            scenario,
            registry,
            slot_state,
            predicate,
            children,
            pool,
            aliases,
        })
    }

    async fn execute(&self) -> Result<ScenarioReport> {
        let total = self.scenario.steps.len();
        let mut step_results = Vec::with_capacity(total);
        let mut passed = 0;

        for (idx, step) in self.scenario.steps.iter().enumerate() {
            let failures = self.execute_step(step).await?;
            let step_passed = failures.is_empty();
            if step_passed {
                passed += 1;
            }
            step_results.push(StepResult {
                step_index: idx,
                step_name: step.name.clone(),
                failures,
            });
        }

        Ok(ScenarioReport {
            scenario_name: self.scenario.name.clone(),
            suite_id: self.scenario.suite_id.clone(),
            steps_total: total,
            steps_passed: passed,
            step_results,
        })
    }

    async fn execute_step(&self, step: &ScenarioStep) -> Result<Vec<AssertionFailure>> {
        let mut failures = Vec::new();

        // Mutations apply before any assertion ops in the same step.
        if let Some(muts) = &step.mutate {
            self.apply_mutations(muts)?;
        }

        if let Some(op) = &step.check_transition {
            let actual = self.check_transition(op).await?;
            if let Some(expectation) = &step.expect {
                if let Some(expected_violations) = &expectation.violations {
                    failures.extend(check_violations(expected_violations, &actual));
                }
            }
        }

        if let Some(op) = &step.evaluate_derived {
            let actual = self.evaluate_derived(op).await?;
            if let Some(expectation) = &step.expect {
                if let Some(expected) = &expectation.derived {
                    failures.extend(check_derived_value(expected, &actual));
                }
            }
        }

        if let Some(op) = &step.plan_cascade {
            let actual = self.plan_cascade(op).await?;
            if let Some(expectation) = &step.expect {
                if let Some(expected_actions) = &expectation.cascade_actions {
                    let aliases = self.aliases.clone();
                    failures.extend(check_cascade_actions(expected_actions, &actual, |alias| {
                        aliases.get(alias).copied()
                    }));
                }
            }
        }

        Ok(failures)
    }

    fn apply_mutations(&self, mutations: &[StateEntry]) -> Result<()> {
        for m in mutations {
            let id = lookup_alias(&self.aliases, &m.entity)?;
            self.slot_state
                .set(&m.workspace, &m.slot, id, m.state.as_deref());
        }
        Ok(())
    }

    async fn check_transition(&self, op: &CheckTransitionOp) -> Result<Vec<GateViolation>> {
        let entity_id = lookup_alias(&self.aliases, &op.entity)?;
        let gate = GateChecker::new(
            self.registry.clone(),
            self.slot_state.clone(),
            self.predicate.clone(),
        );
        gate.check_transition(
            &op.workspace,
            &op.slot,
            entity_id,
            &op.from,
            &op.to,
            &self.pool,
        )
        .await
    }

    async fn evaluate_derived(&self, op: &EvaluateDerivedOp) -> Result<DerivedStateValue> {
        let host_id = lookup_alias(&self.aliases, &op.host_entity)?;
        let derived = self
            .registry
            .derived_states_for_slot(&op.workspace, &op.slot)
            .into_iter()
            .find(|d| d.id == op.derived_id)
            .ok_or_else(|| {
                anyhow!(
                    "derived state '{}' not found on slot {}.{} in registry",
                    op.derived_id,
                    op.workspace,
                    op.slot
                )
            })?;
        let evaluator =
            DerivedStateEvaluator::new(self.slot_state.clone(), self.predicate.clone());
        evaluator.evaluate(derived, host_id, &self.pool).await
    }

    async fn plan_cascade(&self, op: &PlanCascadeOp) -> Result<Vec<CascadeAction>> {
        let parent_id = lookup_alias(&self.aliases, &op.parent_entity)?;
        let planner = CascadePlanner::new(self.registry.clone(), self.children.clone());
        planner
            .plan_cascade(
                &op.parent_workspace,
                &op.parent_slot,
                parent_id,
                &op.parent_new_state,
                &self.pool,
            )
            .await
    }
}

fn lookup_alias(aliases: &HashMap<String, Uuid>, name: &str) -> Result<Uuid> {
    aliases
        .get(name)
        .copied()
        .ok_or_else(|| anyhow!("entity alias '{}' not declared in entity_aliases", name))
}

/// Build a `PgPool` that defers connection until first query. The harness
/// mocks never invoke a query, so no real connection is ever opened.
fn build_lazy_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://harness-mock-never-connects")
        .expect("connect_lazy with a valid-shaped URL never fails")
}

/// Resolve a path relative to the repo root by walking up from the
/// current crate dir until we find a directory containing `Cargo.toml` AND
/// `rust/` (the workspace root marker).
fn repo_root_join(rel: &str) -> std::path::PathBuf {
    let mut here = std::env::current_dir().expect("cwd");
    loop {
        if here.join("rust").join("Cargo.toml").exists() {
            return here.join(rel);
        }
        if !here.pop() {
            // Fallback: just use the relative path as-is.
            return std::path::PathBuf::from(rel);
        }
    }
}

#[allow(dead_code)]
fn _unused_warning_silencer(_: &()) -> Result<()> {
    bail!("unused helper")
}
