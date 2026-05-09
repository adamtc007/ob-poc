//! Workflow validity harness for ACP/Sage -> REPL workbook code packs.
//!
//! Measures whether declared Domain Pack transitions can produce valid
//! non-mutating execution workbooks and compiled restricted-mutation runbooks.
//! This is intentionally fast and database-free so it can run as a regular
//! regression test.

use std::collections::BTreeSet;

use chrono::{Duration, Utc};
use ob_poc::runbook::{
    build_kyc_update_status_dry_run, compile_restricted_mutation_preflight,
    create_approval_token_for_workbook, prepare_restricted_mutation_preflight,
    record_restricted_mutation_execution_receipt, validate_workbook_for_dry_run,
    DslCoderExecutionMode, KycUpdateStatusDryRunInput, ObservedMutationAnchors,
};
use sem_os_core::domain_pack::DomainPackManifest;
use uuid::{uuid, Uuid};

const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");

#[derive(Debug)]
struct WorkflowValidityRow {
    transition_ref: String,
    dry_run_valid: bool,
    workbook_integrity_valid: bool,
    dsl_coder_valid: bool,
    approval_valid: bool,
    preflight_valid: bool,
    compiled_runbook_valid: bool,
    receipt_valid: bool,
    failure: Option<String>,
}

impl WorkflowValidityRow {
    fn success(&self) -> bool {
        self.dry_run_valid
            && self.workbook_integrity_valid
            && self.dsl_coder_valid
            && self.approval_valid
            && self.preflight_valid
            && self.compiled_runbook_valid
            && self.receipt_valid
    }
}

#[test]
fn workflow_validity_harness_reports_valid_repl_workbook_dsl_pack_rates() {
    let manifest = load_ob_poc_kyc_domain_pack();
    let validation = manifest.validate();
    assert!(
        validation.valid,
        "Domain Pack is invalid: {:?}",
        validation.diagnostics
    );

    let mut rows = Vec::new();
    for (idx, transition) in manifest.allowed_transitions.iter().enumerate() {
        let input = KycUpdateStatusDryRunInput {
            session_id: SESSION_ID,
            case_id: CASE_ID,
            actor_id: "analyst@example.com".to_string(),
            actor_roles: vec!["analyst".to_string()],
            transition_ref: transition.transition_ref.clone(),
            current_state: transition.from_state.clone(),
            requested_state: transition.to_state.clone(),
            configuration_version: format!("config-{}", idx + 1),
            state_snapshot_id: format!("state-snapshot-{}", idx + 1),
            evidence_digest: format!("sha256:case-{}", idx + 1),
            llm_trace_ref: None,
        };

        rows.push(run_workflow_validity_case(&manifest, idx as u64 + 1, input));
    }

    print_report(&rows);

    let total = rows.len();
    assert!(total > 0, "workflow validity harness has no transitions");
    assert_eq!(
        count(&rows, |r| r.dry_run_valid),
        total,
        "dry-run workbook build rate regressed"
    );
    assert_eq!(
        count(&rows, |r| r.dsl_coder_valid),
        total,
        "DSL Coder workbook validation rate regressed"
    );
    assert_eq!(
        count(&rows, |r| r.compiled_runbook_valid),
        total,
        "compiled REPL runbook code-pack validity rate regressed"
    );
    assert_eq!(
        count(&rows, WorkflowValidityRow::success),
        total,
        "end-to-end workflow validity rate regressed"
    );
}

fn run_workflow_validity_case(
    manifest: &DomainPackManifest,
    runbook_version: u64,
    input: KycUpdateStatusDryRunInput,
) -> WorkflowValidityRow {
    let transition_ref = input.transition_ref.clone();
    let mut row = WorkflowValidityRow {
        transition_ref: transition_ref.clone(),
        dry_run_valid: false,
        workbook_integrity_valid: false,
        dsl_coder_valid: false,
        approval_valid: false,
        preflight_valid: false,
        compiled_runbook_valid: false,
        receipt_valid: false,
        failure: None,
    };

    let output = match build_kyc_update_status_dry_run(input) {
        Ok(output) => {
            row.dry_run_valid = true;
            output
        }
        Err(error) => {
            row.failure = Some(format!("dry-run refused: {error:?}"));
            return row;
        }
    };

    if let Err(error) = output.workbook.validate_integrity() {
        row.failure = Some(format!("workbook integrity failed: {error:?}"));
        return row;
    }
    row.workbook_integrity_valid = true;

    if let Err(error) =
        validate_workbook_for_dry_run(&output.workbook, DslCoderExecutionMode::DryRun)
    {
        row.failure = Some(format!("DSL Coder validation failed: {error:?}"));
        return row;
    }
    row.dsl_coder_valid = true;

    let issued_at = Utc::now();
    let token = match create_approval_token_for_workbook(
        &output.workbook,
        "approver@example.com",
        format!("Approve dry-run workbook {}", output.workbook.id),
        issued_at + Duration::hours(1),
        issued_at,
    ) {
        Ok(token) => {
            row.approval_valid = true;
            token
        }
        Err(error) => {
            row.failure = Some(format!("approval token failed: {error:?}"));
            return row;
        }
    };

    let observed = ObservedMutationAnchors {
        configuration_version: output.workbook.core.configuration_version.clone(),
        state_snapshot_id: output.workbook.core.state_snapshot_id.clone(),
        evidence_refs: output.workbook.core.evidence_refs.clone(),
    };
    let mut mutation_manifest = manifest.clone();
    for transition in &mut mutation_manifest.allowed_transitions {
        if transition.transition_ref == transition_ref {
            transition.mutation_enabled = true;
        }
    }
    let consumed_token_ids = BTreeSet::new();
    let preflight = match prepare_restricted_mutation_preflight(
        &output.workbook,
        Some(&token),
        &mutation_manifest,
        &observed,
        &consumed_token_ids,
        issued_at,
    ) {
        Ok(preflight) => {
            row.preflight_valid = true;
            preflight
        }
        Err(error) => {
            row.failure = Some(format!("restricted mutation preflight failed: {error:?}"));
            return row;
        }
    };

    let compilation =
        match compile_restricted_mutation_preflight(SESSION_ID, runbook_version, &preflight) {
            Ok(compilation) => {
                row.compiled_runbook_valid = true;
                compilation
            }
            Err(error) => {
                row.failure = Some(format!("compiled runbook failed: {error:?}"));
                return row;
            }
        };

    match record_restricted_mutation_execution_receipt(
        &compilation,
        &preflight,
        preflight.intended_diff.clone(),
        issued_at,
    ) {
        Ok(_) => row.receipt_valid = true,
        Err(error) => {
            row.failure = Some(format!("receipt binding failed: {error:?}"));
        }
    }

    row
}

fn load_ob_poc_kyc_domain_pack() -> DomainPackManifest {
    let raw = include_str!("../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml");
    serde_yaml::from_str(raw).expect("bundled ob-poc KYC Domain Pack parses")
}

fn count(rows: &[WorkflowValidityRow], pred: impl Fn(&WorkflowValidityRow) -> bool) -> usize {
    rows.iter().filter(|row| pred(row)).count()
}

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 * 100.0 / total as f64
    }
}

fn print_report(rows: &[WorkflowValidityRow]) {
    let total = rows.len();
    println!("\n=======================================================================");
    println!(
        "  WORKFLOW VALIDITY HARNESS -- {} transition code packs",
        total
    );
    println!("=======================================================================");
    println!(
        "  Dry-run workbook valid:      {}/{} ({:.1}%)",
        count(rows, |r| r.dry_run_valid),
        total,
        pct(count(rows, |r| r.dry_run_valid), total)
    );
    println!(
        "  Workbook integrity valid:    {}/{} ({:.1}%)",
        count(rows, |r| r.workbook_integrity_valid),
        total,
        pct(count(rows, |r| r.workbook_integrity_valid), total)
    );
    println!(
        "  DSL Coder valid:             {}/{} ({:.1}%)",
        count(rows, |r| r.dsl_coder_valid),
        total,
        pct(count(rows, |r| r.dsl_coder_valid), total)
    );
    println!(
        "  Approval/preflight valid:    {}/{} ({:.1}%)",
        count(rows, |r| r.approval_valid && r.preflight_valid),
        total,
        pct(
            count(rows, |r| r.approval_valid && r.preflight_valid),
            total
        )
    );
    println!(
        "  Compiled REPL code packs:    {}/{} ({:.1}%)",
        count(rows, |r| r.compiled_runbook_valid),
        total,
        pct(count(rows, |r| r.compiled_runbook_valid), total)
    );
    println!(
        "  End-to-end valid workflows:  {}/{} ({:.1}%)",
        count(rows, WorkflowValidityRow::success),
        total,
        pct(count(rows, WorkflowValidityRow::success), total)
    );

    for row in rows {
        let status = if row.success() { "ok" } else { "failed" };
        println!("  - {}: {}", row.transition_ref, status);
        if let Some(failure) = &row.failure {
            println!("      {}", failure);
        }
    }
    println!("=======================================================================\n");
}
