//! Sage coverage harness for deterministic intent classification.
//!
//! Usage:
//!   RUSTC_WRAPPER= cargo test -p ob-poc --test sage_coverage -- --ignored --nocapture

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ob_poc::sage::{
    DeterministicSage, IntentPolarity, LlmSage, ObservationPlane, SageContext, SageEngine,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TestFixture {
    #[serde(rename = "test")]
    tests: Vec<TestCase>,
}

#[derive(Debug, Deserialize, Clone)]
struct TestCase {
    utterance: String,
    expected_verb: String,
    category: String,
    difficulty: String,
    #[serde(default)]
    expected_plane: Option<String>,
    #[serde(default)]
    expected_polarity: Option<String>,
    #[serde(default)]
    expected_domain_concept: Option<String>,
}

#[derive(Debug)]
struct CoverageRow {
    utterance: String,
    expected_verb: String,
    expected_plane: Option<String>,
    actual_plane: String,
    expected_polarity: Option<String>,
    actual_polarity: String,
    expected_domain_concept: Option<String>,
    actual_domain_concept: String,
    plane_hit: bool,
    polarity_hit: bool,
    domain_hit: bool,
    category: String,
    difficulty: String,
}

#[derive(Debug, Clone, Copy)]
struct Accuracy {
    hits: usize,
    total: usize,
}

impl Accuracy {
    fn ratio(self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.hits as f64 / self.total as f64
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct DomainBucket {
    hits: usize,
    total: usize,
}

impl DomainBucket {
    fn ratio(self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.hits as f64 / self.total as f64
        }
    }
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/intent_test_utterances.toml")
}

fn output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/sage-coverage")
}

fn load_fixture() -> anyhow::Result<TestFixture> {
    let raw = fs::read_to_string(fixture_path())?;
    let fixture = toml::from_str::<TestFixture>(&raw)?;
    Ok(fixture)
}

fn build_sage_engine() -> Arc<dyn SageEngine> {
    if std::env::var("SAGE_LLM").ok().as_deref() == Some("1") {
        if let Ok(client) = ob_agentic::client_factory::create_llm_client() {
            return Arc::new(LlmSage::new(client));
        }
    }

    Arc::new(DeterministicSage)
}

fn parse_plane(value: &str) -> Option<ObservationPlane> {
    match value.trim().to_ascii_lowercase().as_str() {
        "instance" => Some(ObservationPlane::Instance),
        "structure" => Some(ObservationPlane::Structure),
        "registry" => Some(ObservationPlane::Registry),
        _ => None,
    }
}

fn parse_polarity(value: &str) -> Option<IntentPolarity> {
    match value.trim().to_ascii_lowercase().as_str() {
        "read" => Some(IntentPolarity::Read),
        "write" => Some(IntentPolarity::Write),
        "ambiguous" => Some(IntentPolarity::Ambiguous),
        _ => None,
    }
}

fn context_for_case(case: &TestCase) -> SageContext {
    let stage_focus = match case.expected_plane.as_deref().and_then(parse_plane) {
        Some(ObservationPlane::Structure) => Some("semos-data-management".to_string()),
        Some(ObservationPlane::Registry) => Some("semos-stewardship".to_string()),
        Some(ObservationPlane::Instance) | None => None,
    };

    SageContext {
        session_id: None,
        stage_focus,
        goals: Vec::new(),
        entity_kind: None,
        dominant_entity_name: None,
        last_intents: Vec::new(),
    }
}

fn write_report(
    rows: &[CoverageRow],
    plane: Accuracy,
    polarity: Accuracy,
    domain: Accuracy,
) -> anyhow::Result<()> {
    let out_dir = output_dir();
    fs::create_dir_all(&out_dir)?;

    let mut mismatches = rows
        .iter()
        .filter(|row| !row.plane_hit || !row.polarity_hit || !row.domain_hit)
        .collect::<Vec<_>>();
    mismatches.sort_by(|a, b| {
        a.expected_verb
            .cmp(&b.expected_verb)
            .then(a.utterance.cmp(&b.utterance))
    });

    let mut markdown = String::new();
    markdown.push_str("# Sage Coverage Report\n\n");
    markdown.push_str(&format!(
        "- Plane accuracy: {}/{} ({:.1}%)\n- Polarity accuracy: {}/{} ({:.1}%)\n- Domain accuracy: {}/{} ({:.1}%)\n\n",
        plane.hits,
        plane.total,
        plane.ratio() * 100.0,
        polarity.hits,
        polarity.total,
        polarity.ratio() * 100.0,
        domain.hits,
        domain.total,
        domain.ratio() * 100.0,
    ));

    let mut by_expected_domain = std::collections::BTreeMap::<String, DomainBucket>::new();
    for row in rows
        .iter()
        .filter(|row| row.expected_domain_concept.is_some())
    {
        let key = row.expected_domain_concept.clone().unwrap_or_default();
        let bucket = by_expected_domain.entry(key).or_default();
        bucket.total += 1;
        if row.domain_hit {
            bucket.hits += 1;
        }
    }

    markdown.push_str("## Domain Accuracy By Expected Domain\n\n");
    markdown.push_str("| Domain | Hits | Total | Accuracy |\n");
    markdown.push_str("| --- | --- | --- | --- |\n");
    for (domain_name, bucket) in by_expected_domain {
        markdown.push_str(&format!(
            "| {} | {} | {} | {:.1}% |\n",
            domain_name,
            bucket.hits,
            bucket.total,
            bucket.ratio() * 100.0
        ));
    }
    markdown.push('\n');

    markdown.push_str("## Mismatches\n\n");
    if mismatches.is_empty() {
        markdown.push_str("No mismatches.\n");
    } else {
        markdown.push_str("| Utterance | Expected Verb | Category | Difficulty | Plane | Actual Plane | Polarity | Actual Polarity | Domain | Actual Domain |\n");
        markdown.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
        for row in mismatches {
            markdown.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                row.utterance.replace('|', "\\|"),
                row.expected_verb,
                row.category,
                row.difficulty,
                row.expected_plane.as_deref().unwrap_or("-"),
                row.actual_plane,
                row.expected_polarity.as_deref().unwrap_or("-"),
                row.actual_polarity,
                row.expected_domain_concept.as_deref().unwrap_or("-"),
                if row.actual_domain_concept.is_empty() {
                    "-"
                } else {
                    row.actual_domain_concept.as_str()
                },
            ));
        }
    }

    fs::write(out_dir.join("report.md"), markdown)?;
    Ok(())
}

#[tokio::test]
#[ignore]
async fn sage_coverage() -> anyhow::Result<()> {
    let fixture = load_fixture()?;
    let sage = build_sage_engine();
    let mut rows = Vec::new();
    let mut plane = Accuracy { hits: 0, total: 0 };
    let mut polarity = Accuracy { hits: 0, total: 0 };
    let mut domain = Accuracy { hits: 0, total: 0 };

    for case in fixture.tests {
        let context = context_for_case(&case);
        let outcome = sage.classify(&case.utterance, &context).await?;

        let expected_plane = case.expected_plane.clone();
        let expected_polarity = case.expected_polarity.clone();
        let expected_domain_concept = case.expected_domain_concept.clone();

        let plane_hit = expected_plane
            .as_deref()
            .and_then(parse_plane)
            .map(|expected| {
                plane.total += 1;
                let hit = outcome.plane == expected;
                if hit {
                    plane.hits += 1;
                }
                hit
            })
            .unwrap_or(false);

        let polarity_hit = expected_polarity
            .as_deref()
            .and_then(parse_polarity)
            .map(|expected| {
                polarity.total += 1;
                let hit = outcome.polarity == expected;
                if hit {
                    polarity.hits += 1;
                }
                hit
            })
            .unwrap_or(false);

        let domain_hit = expected_domain_concept
            .as_deref()
            .map(|expected| {
                domain.total += 1;
                let hit = outcome.domain_concept == expected;
                if hit {
                    domain.hits += 1;
                }
                hit
            })
            .unwrap_or(false);

        rows.push(CoverageRow {
            utterance: case.utterance,
            expected_verb: case.expected_verb,
            expected_plane,
            actual_plane: outcome.plane.as_str().to_string(),
            expected_polarity,
            actual_polarity: outcome.polarity.as_str().to_string(),
            expected_domain_concept,
            actual_domain_concept: outcome.domain_concept,
            plane_hit,
            polarity_hit,
            domain_hit,
            category: case.category,
            difficulty: case.difficulty,
        });
    }

    write_report(&rows, plane, polarity, domain)?;

    println!(
        "plane accuracy: {}/{} ({:.1}%)",
        plane.hits,
        plane.total,
        plane.ratio() * 100.0
    );
    println!(
        "polarity accuracy: {}/{} ({:.1}%)",
        polarity.hits,
        polarity.total,
        polarity.ratio() * 100.0
    );
    println!(
        "domain accuracy: {}/{} ({:.1}%)",
        domain.hits,
        domain.total,
        domain.ratio() * 100.0
    );
    println!("report: {}", output_dir().join("report.md").display());

    assert!(plane.ratio() >= 0.70, "plane accuracy below gate");
    assert!(polarity.ratio() >= 0.80, "polarity accuracy below gate");
    assert!(domain.ratio() >= 0.60, "domain accuracy below gate");

    Ok(())
}
