//! Live-DB cross-workspace DAG scenario tests.
//!
//! Each `#[sqlx::test]` function gets a fresh ephemeral Postgres database
//! with `rust/test-migrations/cross_workspace_dag/0001_schema.sql` applied.
//! sqlx drops the DB after the test → no rollback needed.
//!
//! Add a new live scenario:
//!   1. Author the YAML in `tests/fixtures/cross_workspace_dag/`
//!      (or extend an existing fixture with `mode: live` / `mode: both`).
//!   2. Append a `live_scenario_test!` line below.
//!
//! Run: cargo test -p dsl-runtime --features harness --test cross_workspace_dag_live_scenarios

use dsl_runtime::cross_workspace::test_harness::LiveScenarioRunner;
use sqlx::PgPool;

macro_rules! live_scenario_test {
    ($name:ident, $path:literal) => {
        #[sqlx::test(migrations = "../../test-migrations/cross_workspace_dag")]
        async fn $name(pool: PgPool) {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/", $path);
            let report = LiveScenarioRunner::run_scenario_file(path, pool)
                .await
                .expect("live scenario runner failed to load/execute");
            assert!(
                report.passed(),
                "live scenario failed:\n{}",
                report.failure_summary()
            );
        }
    };
}

// Phase 1 MVP: one fixture proven live. Each row exercises real
// PostgresSlotStateProvider + SqlPredicateResolver against a per-test
// ephemeral DB. Phase 2 expands to all 6 existing fixtures + adds
// new ones authored against the test schema.
live_scenario_test!(
    live_deal_contracted_compound_tollgate,
    "tests/fixtures/cross_workspace_dag/deal_contracted_compound_tollgate_live.yaml"
);
