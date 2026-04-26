//! Cross-workspace DAG scenario tests.
//!
//! Each `scenario_test!` invocation binds one fixture YAML to a `#[tokio::test]`
//! function. Add a new fixture: drop it in `tests/fixtures/cross_workspace_dag/`
//! and append a `scenario_test!` line below.
//!
//! Run all: `cargo test -p dsl-runtime --features harness --test cross_workspace_dag_scenarios`
//! Run one: `cargo test -p dsl-runtime --features harness --test cross_workspace_dag_scenarios deal_contracted`

use dsl_runtime::cross_workspace::test_harness::ScenarioRunner;

macro_rules! scenario_test {
    ($name:ident, $path:literal) => {
        #[tokio::test]
        async fn $name() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/", $path);
            let report = ScenarioRunner::run_scenario_file(path)
                .await
                .expect("scenario runner failed to load/execute");
            assert!(
                report.passed(),
                "scenario failed:\n{}",
                report.failure_summary()
            );
        }
    };
}

scenario_test!(
    deal_contracted_compound_tollgate,
    "tests/fixtures/cross_workspace_dag/deal_contracted_compound_tollgate.yaml"
);

scenario_test!(
    cbu_validated_requires_kyc_approved,
    "tests/fixtures/cross_workspace_dag/cbu_validated_requires_kyc_approved.yaml"
);

scenario_test!(
    im_mandate_requires_validated_cbu,
    "tests/fixtures/cross_workspace_dag/im_mandate_requires_validated_cbu.yaml"
);

scenario_test!(
    cbu_operationally_active_aggregate,
    "tests/fixtures/cross_workspace_dag/cbu_operationally_active_aggregate.yaml"
);

scenario_test!(
    four_layer_chain_end_to_end,
    "tests/fixtures/cross_workspace_dag/four_layer_chain_end_to_end.yaml"
);

scenario_test!(
    business_flow_deal_to_resources,
    "tests/fixtures/cross_workspace_dag/business_flow_deal_to_resources.yaml"
);
