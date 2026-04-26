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

// Each test exercises real PostgresSlotStateProvider + SqlPredicateResolver
// against a per-test ephemeral DB.
//
// Two existing mock-mode fixtures (cbu_validated_requires_kyc_approved,
// cbu_operationally_active_aggregate) cannot be ported to live mode yet —
// SqlPredicateResolver only handles simple-equality predicates; their
// constraints use OR / EXISTS / multi-line block predicates the parser
// returns None for. Production note: if either of those constraints
// actually fires in production, it'd silently classify as "predicate
// didn't resolve" → constraint violation. Tracked as tech debt for the
// production resolver, not a harness gap.
live_scenario_test!(
    live_deal_contracted_compound_tollgate,
    "tests/fixtures/cross_workspace_dag/deal_contracted_compound_tollgate_live.yaml"
);

live_scenario_test!(
    live_im_mandate_requires_validated_cbu,
    "tests/fixtures/cross_workspace_dag/im_mandate_requires_validated_cbu_live.yaml"
);

live_scenario_test!(
    live_four_layer_chain_end_to_end,
    "tests/fixtures/cross_workspace_dag/four_layer_chain_end_to_end_live.yaml"
);
