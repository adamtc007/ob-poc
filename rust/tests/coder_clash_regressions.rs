//! Regression checks for known high-value clash pairs exported by the Coder clash matrix.

use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Fixture {
    case: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    verb_a: String,
    verb_b: String,
    expected_kind: String,
    notes: Option<String>,
}

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/coder_clash_regressions.toml")
}

fn clash_csv_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/coder-clash-matrix/clash_matrix.csv")
}

#[test]
fn exported_clash_matrix_contains_regression_pairs() -> anyhow::Result<()> {
    let fixture: Fixture = toml::from_str(&fs::read_to_string(fixture_path())?)?;
    let csv = fs::read_to_string(clash_csv_path())?;

    for case in fixture.case {
        let needle_a = format!("{},{}", case.verb_a, case.verb_b);
        let needle_b = format!("{},{}", case.verb_b, case.verb_a);
        let matching_line = csv
            .lines()
            .find(|line| {
                (line.starts_with(&needle_a) || line.starts_with(&needle_b))
                    && line.contains(&case.expected_kind)
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "missing clash regression pair {} / {} with kind {} ({})",
                    case.verb_a,
                    case.verb_b,
                    case.expected_kind,
                    case.notes.unwrap_or_default()
                )
            })?;
        assert!(
            matching_line.contains(&case.expected_kind),
            "expected clash kind {} for {} / {}",
            case.expected_kind,
            case.verb_a,
            case.verb_b
        );
    }

    Ok(())
}
