use std::path::PathBuf;

use ob_poc::agent::harness::load_suite;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/nlci")
        .join(name)
}

#[test]
fn nlci_cbu_harness_suite_loads() {
    let suite = load_suite(&fixture_path("cbu_compiler_harness.yaml")).expect("suite should load");

    assert_eq!(suite.suite_id, "nlci_cbu_compiler");
    assert_eq!(suite.scenarios.len(), 12);
    assert_eq!(suite.scenarios[0].name, "CBU List");
    assert_eq!(
        suite.scenarios[0].steps[0].expect.chosen_verb.as_deref(),
        Some("cbu.list")
    );
    assert_eq!(
        suite.scenarios[1].steps[0].expect.outcome.as_deref(),
        Some("NeedsUserInput")
    );
    assert_eq!(
        suite.scenarios[2].steps[0].expect.outcome.as_deref(),
        Some("NeedsUserInput")
    );
    assert_eq!(
        suite.scenarios[3].steps[0].expect.outcome.as_deref(),
        Some("NeedsUserInput")
    );
    assert_eq!(
        suite.scenarios[4].steps[0].expect.outcome.as_deref(),
        Some("NeedsUserInput")
    );
    assert_eq!(
        suite.scenarios[5].steps[0].expect.outcome.as_deref(),
        Some("NeedsUserInput")
    );
    assert_eq!(
        suite.scenarios[6].steps[0].expect.outcome.as_deref(),
        Some("NeedsUserInput")
    );
    assert_eq!(
        suite.scenarios[7].steps[0].expect.outcome.as_deref(),
        Some("NeedsUserInput")
    );
    assert_eq!(
        suite.scenarios[8].steps[0].expect.outcome.as_deref(),
        Some("NeedsUserInput")
    );
    assert_eq!(
        suite.scenarios[9].steps[0].expect.outcome.as_deref(),
        Some("NeedsUserInput")
    );
    assert_eq!(
        suite.scenarios[10].steps[0].expect.outcome.as_deref(),
        Some("NeedsUserInput")
    );
    assert_eq!(
        suite.scenarios[11].steps[0].expect.outcome.as_deref(),
        Some("NeedsUserInput")
    );
}
