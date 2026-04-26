//! P0 Regression test for F14: Tier -2A ScenarioIndex SemOS-bypass.
//!
//! Reads `tests/fixtures/p0_bypass/p0_bypass_regression.toml` and exercises
//! each case against a minimal `HybridVerbSearcher`. Before the F14 fix, the
//! scenario-matched path at `rust/src/mcp/verb_search.rs:630` pushed route
//! FQNs without consulting `allowed_verbs`. This test pins the fixed
//! behaviour: scenario-routed verbs that are not in `allowed_verbs` must not
//! appear in search results.
//!
//! Fixture format documented at the head of the TOML file.

use std::collections::HashSet;
use std::sync::Arc;

use ob_poc::mcp::scenario_index::ScenarioIndex;
use ob_poc::mcp::verb_search::HybridVerbSearcher;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Fixture {
    #[serde(rename = "case")]
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    name: String,
    utterance: String,
    scenarios_yaml: String,
    #[serde(default)]
    allowed_verbs: Option<Vec<String>>,
    /// "verb:<FQN>" (verb must appear) or "filtered" (no result or the
    /// scenario verb must not appear).
    expected: String,
}

fn load_fixture() -> Fixture {
    let raw = include_str!("fixtures/p0_bypass/p0_bypass_regression.toml");
    toml::from_str(raw).expect("p0_bypass_regression.toml must parse")
}

#[tokio::test]
async fn p0_bypass_regression_all_cases() {
    let fixture = load_fixture();
    assert!(
        !fixture.cases.is_empty(),
        "fixture should have at least one case"
    );

    let mut failures: Vec<String> = Vec::new();

    for case in &fixture.cases {
        let idx = ScenarioIndex::from_yaml_str(&case.scenarios_yaml)
            .unwrap_or_else(|e| panic!("case {}: failed to load scenarios_yaml: {}", case.name, e));
        let searcher = HybridVerbSearcher::minimal().with_scenario_index(Arc::new(idx));

        // Fixture's allowed_verbs list → Option<&HashSet<String>>
        let allowed_set: Option<HashSet<String>> = case
            .allowed_verbs
            .as_ref()
            .map(|v| v.iter().cloned().collect());
        let allowed_ref = allowed_set.as_ref();

        let results = searcher
            .search(
                &case.utterance,
                None,
                None,
                None,
                5,
                allowed_ref,
                None,
                None,
            )
            .await
            .unwrap_or_else(|e| panic!("case {}: search() errored: {}", case.name, e));

        let outcome = if let Some(stripped) = case.expected.strip_prefix("verb:") {
            let expected_fqn = stripped;
            let hit = results.iter().any(|r| r.verb == expected_fqn);
            if hit {
                Ok(())
            } else {
                Err(format!(
                    "expected verb {expected_fqn} but got {:?}",
                    results.iter().map(|r| &r.verb).collect::<Vec<_>>()
                ))
            }
        } else if case.expected == "filtered" {
            // Fail-closed assertion: the scenario FQN in the fixture MUST NOT
            // appear in results. (Other unrelated verbs from lower tiers might
            // appear — but the minimal searcher has no lower-tier sources, so
            // a true bypass regression would put the scenario FQN in results.)
            if results.is_empty() {
                Ok(())
            } else {
                // Parse the scenario FQN out of the YAML (look for `macro_fqn:` lines).
                let scenario_fqns: Vec<&str> = case
                    .scenarios_yaml
                    .lines()
                    .filter_map(|line| line.trim().strip_prefix("macro_fqn:").map(|v| v.trim()))
                    .collect();

                let bypass_hit = results
                    .iter()
                    .find(|r| scenario_fqns.iter().any(|f| f == &r.verb));
                if let Some(r) = bypass_hit {
                    Err(format!(
                        "P0 bypass regressed: scenario FQN {} leaked past \
                         allowed_verbs filter (full result set: {:?})",
                        r.verb,
                        results.iter().map(|r| &r.verb).collect::<Vec<_>>()
                    ))
                } else {
                    Ok(())
                }
            }
        } else {
            panic!(
                "case {}: unrecognised expected format {:?} — must be 'verb:<FQN>' or 'filtered'",
                case.name, case.expected
            );
        };

        if let Err(msg) = outcome {
            failures.push(format!("[{}] {}", case.name, msg));
        }
    }

    assert!(
        failures.is_empty(),
        "P0 bypass regression: {} case(s) failed:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}
