//! Regression checks for known high-value clash pairs in the Coder clash matrix.

use std::fs;
use std::path::Path;

use ob_poc::sage::{build_clash_matrix, VerbMetadataIndex};
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

#[test]
fn exported_clash_matrix_contains_regression_pairs() -> anyhow::Result<()> {
    let fixture: Fixture = toml::from_str(&fs::read_to_string(fixture_path())?)?;
    let index = VerbMetadataIndex::load()?;
    let rows = build_clash_matrix(&index);

    for case in fixture.case {
        let matching_row = rows
            .iter()
            .find(|row| {
                ((row.verb_a == case.verb_a && row.verb_b == case.verb_b)
                    || (row.verb_a == case.verb_b && row.verb_b == case.verb_a))
                    && format!("{:?}", row.clash_kind) == case.expected_kind
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
        assert_eq!(format!("{:?}", matching_row.clash_kind), case.expected_kind);
    }

    Ok(())
}
