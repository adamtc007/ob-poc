//! Deterministic language-acquisition loop harness for KYC update-status.

use std::fs;
use std::path::Path;

use crate::runbook::{
    build_kyc_update_status_language_pack, run_kyc_update_status_revision_loop,
    validate_kyc_update_status_draft_without_revision, KycLanguagePackRequest,
    KycUpdateStatusWorkbookDraft, WorkbookRevisionOutcome,
};
use sem_os_policy::domain_pack::DomainPackManifest;
use serde::Deserialize;
use uuid::{uuid, Uuid};

const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");
const FIXTURE_DIR: &str = "tests/fixtures/workflow_validity/kyc_update_status";

#[derive(Debug, Deserialize)]
struct Fixture {
    name: String,
    language_current_state: String,
    #[serde(default)]
    disable_dry_run_for_transition: Option<String>,
    draft: KycUpdateStatusWorkbookDraft,
    expected: Expected,
}

#[derive(Debug, Deserialize)]
struct Expected {
    outcome: String,
    #[serde(default)]
    revision_count: Option<u8>,
    #[serde(default)]
    refusal_code: Option<String>,
}

#[test]
fn workflow_language_loop_harness_reports_draft_revision_dry_run_rates() {
    let fixtures = load_fixtures();
    assert!(fixtures.len() >= 20, "expected at least 20 fixtures");

    let mut total = 0usize;
    let mut baseline_valid = 0usize;
    let mut baseline_invalid = 0usize;
    let mut uplifted_to_valid = 0usize;
    let mut contained_invalid = 0usize;
    let mut first_pass_valid = 0usize;
    let mut revised_valid = 0usize;
    let mut dry_run_valid = 0usize;
    let mut refused = 0usize;
    let mut invented_verb_count = 0u32;
    let mut measured_dry_run_us = 0u64;
    let mut max_dry_run_us = 0u64;

    for fixture in fixtures {
        total += 1;
        let mut manifest = load_ob_poc_kyc_domain_pack();
        if let Some(disabled_ref) = &fixture.disable_dry_run_for_transition {
            for transition in &mut manifest.allowed_transitions {
                if &transition.transition_ref == disabled_ref {
                    transition.dry_run_enabled = false;
                }
            }
        }

        let pack = build_kyc_update_status_language_pack(
            &manifest,
            KycLanguagePackRequest {
                subject_id: CASE_ID,
                current_state: fixture.language_current_state.clone(),
                configuration_version: "config-1".to_string(),
                state_snapshot_id: "snapshot-1".to_string(),
                objective: None,
            },
        )
        .expect("language pack");

        let baseline =
            validate_kyc_update_status_draft_without_revision(&manifest, &pack, &fixture.draft);
        let baseline_was_valid = baseline.is_ok();
        if baseline_was_valid {
            baseline_valid += 1;
        } else {
            baseline_invalid += 1;
        }

        let outcome = run_kyc_update_status_revision_loop(&manifest, &pack, fixture.draft);
        match &outcome {
            WorkbookRevisionOutcome::DryRunValid { metrics, trace, .. } => {
                assert_eq!(
                    fixture.expected.outcome, "dry_run_valid",
                    "{}",
                    fixture.name
                );
                dry_run_valid += 1;
                if metrics.first_pass_valid {
                    first_pass_valid += 1;
                }
                if metrics.revision_count > 0 {
                    revised_valid += 1;
                    if !baseline_was_valid {
                        uplifted_to_valid += 1;
                    }
                }
                if let Some(expected_revisions) = fixture.expected.revision_count {
                    assert_eq!(
                        metrics.revision_count, expected_revisions,
                        "{}",
                        fixture.name
                    );
                }
                assert!(trace.iter().any(|event| event.phase == "language_pack"));
                assert!(trace.iter().any(|event| event.phase == "dry_run"));
                measured_dry_run_us += metrics.dry_run_us;
                max_dry_run_us = max_dry_run_us.max(metrics.dry_run_us);
                invented_verb_count += metrics.invented_verb_count;
            }
            WorkbookRevisionOutcome::Refused {
                refusal,
                metrics,
                trace,
                ..
            } => {
                assert_eq!(fixture.expected.outcome, "refused", "{}", fixture.name);
                refused += 1;
                if !baseline_was_valid {
                    contained_invalid += 1;
                }
                let expected_refusal = fixture
                    .expected
                    .refusal_code
                    .as_deref()
                    .expect("refusal fixture has code");
                assert_eq!(refusal.refusal_code, expected_refusal, "{}", fixture.name);
                assert_eq!(
                    metrics.refusal_code.as_deref(),
                    Some(expected_refusal),
                    "{}",
                    fixture.name
                );
                assert!(trace.iter().any(|event| event.phase == "language_pack"));
                assert!(trace.iter().any(|event| event.phase == "refusal"));
                measured_dry_run_us += metrics.dry_run_us;
                max_dry_run_us = max_dry_run_us.max(metrics.dry_run_us);
                invented_verb_count += metrics.invented_verb_count;
            }
        }
    }

    println!("\n=======================================================================");
    println!("  WORKFLOW LANGUAGE LOOP HARNESS -- {} fixtures", total);
    println!("=======================================================================");
    println!(
        "  Baseline strict valid:    {}/{} ({:.1}%)",
        baseline_valid,
        total,
        pct(baseline_valid, total)
    );
    println!(
        "  Baseline strict invalid:  {}/{} ({:.1}%)",
        baseline_invalid,
        total,
        pct(baseline_invalid, total)
    );
    println!(
        "  First-pass valid:         {}/{} ({:.1}%)",
        first_pass_valid,
        total,
        pct(first_pass_valid, total)
    );
    println!(
        "  Revised valid:            {}/{} ({:.1}%)",
        revised_valid,
        total,
        pct(revised_valid, total)
    );
    println!(
        "  Dry-run valid:            {}/{} ({:.1}%)",
        dry_run_valid,
        total,
        pct(dry_run_valid, total)
    );
    println!(
        "  Structured refusals:      {}/{} ({:.1}%)",
        refused,
        total,
        pct(refused, total)
    );
    println!(
        "  Uplifted to valid:        {}/{} ({:.1}%)",
        uplifted_to_valid,
        total,
        pct(uplifted_to_valid, total)
    );
    println!(
        "  Invalid contained:        {}/{} ({:.1}%)",
        contained_invalid,
        total,
        pct(contained_invalid, total)
    );
    println!("  Invented verb count:      {}", invented_verb_count);
    println!(
        "  Avg measured dry_run_ms:  {:.2}",
        avg_ms(measured_dry_run_us, total)
    );
    println!(
        "  Max measured dry_run_ms:  {:.2}",
        max_dry_run_us as f64 / 1_000.0
    );
    println!("=======================================================================\n");

    assert_eq!(baseline_valid, 2);
    assert_eq!(uplifted_to_valid, 8);
    assert_eq!(contained_invalid, 10);
    assert_eq!(first_pass_valid, 2);
    assert_eq!(dry_run_valid, 10);
    assert_eq!(refused, 10);
    assert_eq!(invented_verb_count, 2);
}

fn load_fixtures() -> Vec<Fixture> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_DIR);
    let mut paths = fs::read_dir(&base)
        .unwrap_or_else(|error| panic!("read fixture dir {}: {error}", base.display()))
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let raw = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display()));
            serde_json::from_str(&raw)
                .unwrap_or_else(|error| panic!("parse fixture {}: {error}", path.display()))
        })
        .collect()
}

fn load_ob_poc_kyc_domain_pack() -> DomainPackManifest {
    let raw = include_str!("../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml");
    serde_yaml::from_str(raw).expect("bundled ob-poc KYC Domain Pack parses")
}

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 * 100.0 / total as f64
    }
}

fn avg_ms(total_us: u64, count: usize) -> f64 {
    if count == 0 {
        0.0
    } else {
        total_us as f64 / count as f64 / 1_000.0
    }
}
