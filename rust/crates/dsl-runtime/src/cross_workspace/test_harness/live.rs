//! Live-DB scenario runner. Same scenario YAMLs as the mock runner, but
//! state lookups + predicates + child resolution all go through the
//! production providers against an ephemeral Postgres database created
//! by `#[sqlx::test]`.
//!
//! Each `#[sqlx::test]` test function gets its own fresh database with
//! the migration set under `rust/test-migrations/cross_workspace_dag/`
//! applied. sqlx drops the DB after the test → no rollback needed,
//! cleanup is automatic at the database level.
//!
//! See `tests/cross_workspace_dag_live_scenarios.rs` for the entry point.

use anyhow::{anyhow, Context, Result};
use dsl_core::config::DagRegistry;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use crate::cross_workspace::derived_state::{DerivedStateEvaluator, DerivedStateValue};
use crate::cross_workspace::gate_checker::{GateChecker, GateViolation};
use crate::cross_workspace::hierarchy_cascade::{CascadeAction, CascadePlanner};
use crate::cross_workspace::postgres_child_resolver::PostgresChildEntityResolver;
use crate::cross_workspace::slot_state::PostgresSlotStateProvider;
use crate::cross_workspace::sql_predicate_resolver::SqlPredicateResolver;

use super::assertions::{
    check_cascade_actions, check_derived_value, check_violations, AssertionFailure,
};
use super::runner::{ScenarioReport, StepResult};
use super::scenario::{
    CheckTransitionOp, EvaluateDerivedOp, PlanCascadeOp, Scenario, ScenarioStep, SeedDependency,
    StateEntry,
};

const DEFAULT_DAG_DIR: &str = "rust/config/sem_os_seeds/dag_taxonomies";

/// Live-mode runner. Loaded scenario + per-test pool + production providers.
pub struct LiveScenarioRunner {
    scenario: Scenario,
    registry: Arc<DagRegistry>,
    aliases: HashMap<String, Uuid>,
    pool: PgPool,
}

impl LiveScenarioRunner {
    /// Run a scenario YAML against the supplied per-test PgPool.
    pub async fn run_scenario_file(
        path: impl AsRef<Path>,
        pool: PgPool,
    ) -> Result<ScenarioReport> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading scenario file {}", path.display()))?;
        let scenario: Scenario = serde_yaml::from_str(&text)
            .with_context(|| format!("parsing scenario file {}", path.display()))?;
        Self::run_scenario(scenario, pool).await
    }

    pub async fn run_scenario(scenario: Scenario, pool: PgPool) -> Result<ScenarioReport> {
        let runner = Self::build(scenario, pool)?;
        runner.seed().await?;
        runner.execute().await
    }

    fn build(scenario: Scenario, pool: PgPool) -> Result<Self> {
        let mut aliases = HashMap::new();
        for (name, uuid_str) in &scenario.entity_aliases {
            let id = Uuid::parse_str(uuid_str)
                .with_context(|| format!("alias '{}' = '{}' invalid UUID", name, uuid_str))?;
            aliases.insert(name.clone(), id);
        }

        let dag_dir = scenario
            .dag_taxonomies_dir
            .clone()
            .unwrap_or_else(|| DEFAULT_DAG_DIR.to_string());
        let dag_path = repo_root_join(&dag_dir);
        let registry = Arc::new(
            DagRegistry::from_dir(&dag_path)
                .with_context(|| format!("loading DAG taxonomies from {}", dag_path.display()))?,
        );

        Ok(Self {
            scenario,
            registry,
            aliases,
            pool,
        })
    }

    // ---------------------------------------------------------------
    // Seed engine: SeedDependencies → initial_state → INSERT statements.
    // ---------------------------------------------------------------

    async fn seed(&self) -> Result<()> {
        // 1. Optional seed_dependencies first (FK-parent rows).
        for dep in &self.scenario.seed_dependencies {
            self.insert_seed_dependency(dep).await?;
        }
        // 2. initial_state: each (workspace, slot, entity, state) →
        //    INSERT into the dispatched table with the entity_id and
        //    state column set.
        for entry in &self.scenario.initial_state {
            self.insert_initial_state(entry).await?;
        }
        Ok(())
    }

    async fn insert_seed_dependency(&self, dep: &SeedDependency) -> Result<()> {
        let mut col_names: Vec<String> = Vec::new();
        let mut placeholders: Vec<String> = Vec::new();
        let mut bind_strings: Vec<Option<String>> = Vec::new();
        let mut bind_uuids: Vec<Option<Uuid>> = Vec::new();
        let mut bind_kinds: Vec<BindKind> = Vec::new();

        for (idx, (col, value)) in dep.columns.iter().enumerate() {
            col_names.push(col.clone());
            placeholders.push(format!("${}", idx + 1));
            let (kind, s, u) = self.classify_value(value)?;
            bind_kinds.push(kind);
            bind_strings.push(s);
            bind_uuids.push(u);
        }

        let sql = format!(
            r#"INSERT INTO "ob-poc".{} ({}) VALUES ({})"#,
            dep.table,
            col_names.join(", "),
            placeholders.join(", "),
        );

        let mut q = sqlx::query(&sql);
        for (i, kind) in bind_kinds.iter().enumerate() {
            q = match kind {
                BindKind::Uuid => q.bind(bind_uuids[i]),
                BindKind::String => q.bind(bind_strings[i].clone()),
            };
        }
        q.execute(&self.pool)
            .await
            .with_context(|| format!("inserting seed_dependency into {}", dep.table))?;
        Ok(())
    }

    fn classify_value(
        &self,
        value: &JsonValue,
    ) -> Result<(BindKind, Option<String>, Option<Uuid>)> {
        match value {
            JsonValue::String(s) => {
                if let Some(alias_name) = s.strip_prefix("alias:") {
                    let id = self.lookup_alias(alias_name)?;
                    Ok((BindKind::Uuid, None, Some(id)))
                } else if let Ok(uuid) = Uuid::parse_str(s) {
                    // Bare UUID string → bind as uuid (avoids "is of type uuid
                    // but expression is of type text" errors).
                    Ok((BindKind::Uuid, None, Some(uuid)))
                } else {
                    Ok((BindKind::String, Some(s.clone()), None))
                }
            }
            JsonValue::Number(n) => Ok((BindKind::String, Some(n.to_string()), None)),
            JsonValue::Bool(b) => Ok((BindKind::String, Some(b.to_string()), None)),
            JsonValue::Null => Ok((BindKind::String, None, None)),
            other => Err(anyhow!(
                "unsupported seed value type: {:?} (use string, number, bool, or null)",
                other
            )),
        }
    }

    async fn insert_initial_state(&self, entry: &StateEntry) -> Result<()> {
        let entity_id = self.lookup_alias(&entry.entity)?;
        let (table, state_col, pk_col) = resolve_slot_table(&entry.workspace, &entry.slot)?;

        // Build the INSERT column set:
        //   1. PK column (entity_id from alias)
        //   2. state column (if `state:` provided)
        //   3. table-default minimum columns (NOT NULL parents not covered by attrs)
        //   4. fixture-supplied `attrs` overrides (these win — they're the
        //      explicit FK-bridging values the test author chose)
        let mut col_names: Vec<String> = vec![pk_col.to_string()];
        let mut placeholders: Vec<String> = vec!["$1".into()];
        let mut bind_uuids: Vec<Option<Uuid>> = vec![Some(entity_id)];
        let mut bind_strings: Vec<Option<String>> = vec![None];
        let mut bind_kinds: Vec<BindKind> = vec![BindKind::Uuid];

        let mut seen_cols: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        seen_cols.insert(pk_col.to_string());

        if let Some(state) = &entry.state {
            col_names.push(state_col.to_string());
            placeholders.push(format!("${}", placeholders.len() + 1));
            bind_uuids.push(None);
            bind_strings.push(Some(state.clone()));
            bind_kinds.push(BindKind::String);
            seen_cols.insert(state_col.to_string());
        }

        // Fixture-supplied `attrs` first so we can mark them seen and skip
        // the table-default for those columns.
        for (col, value) in &entry.attrs {
            col_names.push(col.clone());
            placeholders.push(format!("${}", placeholders.len() + 1));
            let (kind, s, u) = self.classify_value(value)?;
            bind_kinds.push(kind);
            bind_uuids.push(u);
            bind_strings.push(s);
            seen_cols.insert(col.clone());
        }

        // Table-default minimums for columns the fixture didn't override.
        for (col, value) in table_minimum_columns_owned(table) {
            if seen_cols.contains(col) {
                continue;
            }
            col_names.push(col.to_string());
            placeholders.push(format!("${}", placeholders.len() + 1));
            let (kind, s, u) = self.classify_value(&value)?;
            bind_kinds.push(kind);
            bind_uuids.push(u);
            bind_strings.push(s);
        }

        let sql = format!(
            r#"INSERT INTO "ob-poc".{} ({}) VALUES ({})"#,
            table,
            col_names.join(", "),
            placeholders.join(", "),
        );

        let mut q = sqlx::query(&sql);
        for (i, kind) in bind_kinds.iter().enumerate() {
            q = match kind {
                BindKind::Uuid => q.bind(bind_uuids[i]),
                BindKind::String => q.bind(bind_strings[i].clone()),
            };
        }
        q.execute(&self.pool)
            .await
            .with_context(|| format!(
                "inserting initial_state into {} (workspace={}, slot={}, entity={}, state={:?})",
                table, entry.workspace, entry.slot, entry.entity, entry.state,
            ))?;
        Ok(())
    }

    // ---------------------------------------------------------------
    // Execute steps using production providers.
    // ---------------------------------------------------------------

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

        if let Some(muts) = &step.mutate {
            self.apply_mutations(muts).await?;
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

    async fn apply_mutations(&self, mutations: &[StateEntry]) -> Result<()> {
        for m in mutations {
            let id = self.lookup_alias(&m.entity)?;
            let (table, state_col, pk_col) = resolve_slot_table(&m.workspace, &m.slot)?;
            let sql = format!(
                r#"UPDATE "ob-poc".{} SET {} = $1 WHERE {} = $2"#,
                table, state_col, pk_col,
            );
            let new_state = m.state.clone();
            sqlx::query(&sql)
                .bind(new_state)
                .bind(id)
                .execute(&self.pool)
                .await
                .with_context(|| format!(
                    "mutating {}.{} for entity={} state={:?}",
                    m.workspace, m.slot, m.entity, m.state
                ))?;
        }
        Ok(())
    }

    async fn check_transition(&self, op: &CheckTransitionOp) -> Result<Vec<GateViolation>> {
        let entity_id = self.lookup_alias(&op.entity)?;
        let provider = Arc::new(PostgresSlotStateProvider);
        let resolver = Arc::new(SqlPredicateResolver);
        let gate = GateChecker::new(self.registry.clone(), provider, resolver);
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
        let host_id = self.lookup_alias(&op.host_entity)?;
        let derived = self
            .registry
            .derived_states_for_slot(&op.workspace, &op.slot)
            .into_iter()
            .find(|d| d.id == op.derived_id)
            .ok_or_else(|| {
                anyhow!(
                    "derived state '{}' not found on {}.{}",
                    op.derived_id,
                    op.workspace,
                    op.slot
                )
            })?;
        let provider = Arc::new(PostgresSlotStateProvider);
        let resolver = Arc::new(SqlPredicateResolver);
        let evaluator = DerivedStateEvaluator::new(provider, resolver);
        evaluator.evaluate(derived, host_id, &self.pool).await
    }

    async fn plan_cascade(&self, op: &PlanCascadeOp) -> Result<Vec<CascadeAction>> {
        let parent_id = self.lookup_alias(&op.parent_entity)?;
        let resolver = Arc::new(PostgresChildEntityResolver::new(self.registry.clone()));
        let planner = CascadePlanner::new(self.registry.clone(), resolver);
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

    fn lookup_alias(&self, name: &str) -> Result<Uuid> {
        self.aliases
            .get(name)
            .copied()
            .ok_or_else(|| anyhow!("entity alias '{}' not in entity_aliases", name))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum BindKind {
    Uuid,
    String,
}

/// Walk up from CWD until we find the rust/ workspace root, then join `rel`.
fn repo_root_join(rel: &str) -> std::path::PathBuf {
    let mut here = std::env::current_dir().expect("cwd");
    loop {
        if here.join("rust").join("Cargo.toml").exists() {
            return here.join(rel);
        }
        if !here.pop() {
            return std::path::PathBuf::from(rel);
        }
    }
}

/// Re-implementation of `slot_state::resolve_slot_table` that's accessible
/// from the live runner. Kept in lockstep with the production dispatch
/// table; if a workspace/slot is added to production, add it here too
/// (or the live runner won't be able to seed initial_state for that slot).
fn resolve_slot_table(
    workspace: &str,
    slot: &str,
) -> Result<(&'static str, &'static str, &'static str)> {
    // Phase 1 MVP coverage: the 4 tables in
    // rust/test-migrations/cross_workspace_dag/0001_schema.sql.
    // Phase 2+ extends as more tables are added to that migration.
    let mapping: &[((&str, &str), (&str, &str, &str))] = &[
        // Phase 1
        (("cbu", "cbu"), ("cbus", "status", "cbu_id")),
        (("kyc", "kyc_case"), ("cases", "status", "case_id")),
        (("deal", "deal"), ("deals", "deal_status", "deal_id")),
        (
            ("booking_principal", "clearance"),
            ("booking_principal_clearances", "clearance_status", "id"),
        ),
        // Phase 2
        (
            ("instrument_matrix", "trading_profile"),
            ("cbu_trading_profiles", "status", "profile_id"),
        ),
        (
            ("cbu", "service_consumption"),
            ("cbu_service_consumption", "status", "consumption_id"),
        ),
        (
            ("product_maintenance", "service"),
            ("services", "lifecycle_status", "service_id"),
        ),
        (
            ("lifecycle_resources", "application_instance"),
            ("application_instances", "lifecycle_status", "id"),
        ),
        (
            ("lifecycle_resources", "capability_binding"),
            ("capability_bindings", "binding_status", "id"),
        ),
    ];
    for ((ws, sl), value) in mapping {
        if *ws == workspace && *sl == slot {
            return Ok(*value);
        }
    }
    Err(anyhow!(
        "live harness: no test-schema mapping for ({}, {}). Add the table \
         to rust/test-migrations/cross_workspace_dag/0001_schema.sql AND \
         the dispatch row in live.rs::resolve_slot_table.",
        workspace,
        slot
    ))
}

/// Per-table minimum-required columns (those NOT NULL without sensible
/// DEFAULTs in the test schema). The seed engine bind these into the
/// INSERT *unless* the fixture's `attrs:` overrides them.
///
/// Values can be:
///   - `JsonValue::String("alias:foo")` → resolves to alias UUID
///   - `JsonValue::String("literal")` → bound as text
///   - `JsonValue::Null` → bound as SQL NULL
fn table_minimum_columns_owned(table: &str) -> Vec<(&'static str, JsonValue)> {
    // Sentinel UUID for NOT NULL columns whose value the fixture didn't
    // supply via `attrs:`. Predicates that join on these columns won't
    // match (which is fine — fixture authors must supply the right value
    // when the test depends on the join).
    const SENTINEL: &str = "00000000-0000-0000-0000-000000000999";

    match table {
        "cbus" => vec![("name", JsonValue::String("test cbu".into()))],
        "deals" => vec![
            ("deal_name", JsonValue::String("test deal".into())),
            ("primary_client_group_id", JsonValue::String(SENTINEL.into())),
        ],
        "cases" => vec![
            ("cbu_id", JsonValue::String(SENTINEL.into())),
            ("case_ref", JsonValue::String("TEST-CASE".into())),
        ],
        "booking_principal_clearances" => vec![
            ("booking_principal_id", JsonValue::String(SENTINEL.into())),
        ],
        // Phase 2 tables
        "cbu_trading_profiles" => vec![],
        "cbu_service_consumption" => vec![
            ("cbu_id", JsonValue::String(SENTINEL.into())),
        ],
        "services" => vec![("name", JsonValue::String("test service".into()))],
        "application_instances" => vec![],
        "capability_bindings" => vec![
            ("application_instance_id", JsonValue::String(SENTINEL.into())),
            ("service_id", JsonValue::String(SENTINEL.into())),
        ],
        _ => vec![],
    }
}
