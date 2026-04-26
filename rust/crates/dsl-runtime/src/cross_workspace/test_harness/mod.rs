//! Cross-workspace DAG test harness.
//!
//! Loads scenario YAMLs that declare initial state, predicate truth tables,
//! and a sequence of operations (`check_transition`, `evaluate_derived`,
//! `plan_cascade`, `mutate`). The runner constructs the real
//! [`DagRegistry`](dsl_core::config::DagRegistry) from the workspace's
//! DAG taxonomy YAMLs, wires in-memory mock providers, fires each
//! operation, and asserts on outcomes.
//!
//! # Quick example (rust pseudocode)
//!
//! ```ignore
//! use dsl_runtime::cross_workspace::test_harness::ScenarioRunner;
//!
//! let report = ScenarioRunner::run_scenario_file(
//!     "tests/fixtures/cross_workspace_dag/deal_contracted_compound_tollgate.yaml"
//! ).await?;
//! assert!(report.passed(), "{}", report.failure_summary());
//! ```
//!
//! # Why a no-real-DB harness?
//!
//! The cross-workspace runtime ([`GateChecker`], [`DerivedStateEvaluator`],
//! [`CascadePlanner`]) all take `&PgPool` because the production
//! providers ([`PostgresSlotStateProvider`], [`SqlPredicateResolver`])
//! query Postgres. The mock providers in this module ignore the pool;
//! the runner builds a `PgPool::connect_lazy(...)` that defers connection
//! until first query — and since no SQL ever runs, no connection ever
//! opens. This unlocks fast deterministic scenario testing without
//! requiring a live database.
//!
//! # Scenario YAML schema
//!
//! See `tests/fixtures/cross_workspace_dag/README.md`.
//!
//! [`GateChecker`]: crate::cross_workspace::gate_checker::GateChecker
//! [`DerivedStateEvaluator`]: crate::cross_workspace::derived_state::DerivedStateEvaluator
//! [`CascadePlanner`]: crate::cross_workspace::hierarchy_cascade::CascadePlanner
//! [`PostgresSlotStateProvider`]: crate::cross_workspace::slot_state::PostgresSlotStateProvider
//! [`SqlPredicateResolver`]: crate::cross_workspace::sql_predicate_resolver::SqlPredicateResolver

pub mod assertions;
pub mod live;
pub mod mocks;
pub mod runner;
pub mod scenario;

pub use assertions::AssertionFailure;
pub use live::LiveScenarioRunner;
pub use mocks::{MockChildEntityResolver, MockPredicateResolver, MockSlotStateProvider};
pub use runner::{ScenarioReport, ScenarioRunner, StepResult};
pub use scenario::{
    CheckTransitionOp, ChildEntry, EvaluateDerivedOp, ExpectedCascadeAction, ExpectedCondition,
    ExpectedDerivedValue, ExpectedViolation, PlanCascadeOp, PredicateEntry, Scenario, ScenarioMode,
    ScenarioStep, SeedDependency, StateEntry, StepExpectation,
};
