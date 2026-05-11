//! ACP coverage ledger for authored DSL/DAG workspace surfaces.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use ob_poc::api::acp_dsl_dag_coverage::{
    build_acp_dsl_dag_coverage_report, write_acp_dsl_dag_coverage_artifacts,
    AcpDslDagCoverageStatus, ACP_DSL_DAG_COVERAGE_SCHEMA_VERSION,
};

fn config_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("config")
}

fn output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/acp-dsl-dag-coverage")
}

#[test]
fn acp_dsl_dag_coverage_reports_current_gap_without_prose_escape() -> Result<()> {
    let report = build_acp_dsl_dag_coverage_report(config_root())?;

    assert_eq!(report.schema_version, ACP_DSL_DAG_COVERAGE_SCHEMA_VERSION);
    assert_eq!(report.summary.provider_count, 2);
    assert!(report.summary.pack_count >= 12);
    assert!(report.summary.dag_taxonomy_count >= 12);
    assert!(report.summary.state_machine_count >= 9);
    assert!(report.summary.verb_config_count >= 100);
    assert!(report.summary.total_rows > 100);
    assert!(report.summary.core_acp_covered_rows > 0);
    assert!(report.summary.full_loop_covered_rows > 0);
    assert!(report.summary.core_uncovered_rows > 0);
    assert!(report.summary.full_loop_uncovered_rows > 0);
    assert_eq!(report.summary.prose_only_failure_count, 0);

    assert!(
        report.rows.iter().any(|row| {
            row.verb == "kyc-case.update-status"
                && row.full_loop_covered
                && row.coverage_status == AcpDslDagCoverageStatus::Covered
        }),
        "KYC update-status should still be fully covered by the existing ACP loop"
    );
    assert!(
        report.rows.iter().any(|row| {
            row.verb == "deal.update-status"
                && row.core_acp_covered
                && !row.llm_revision_harness_supported
                && row.coverage_status == AcpDslDagCoverageStatus::CoreOnly
        }),
        "deal.update-status should be core ACP-wired but not yet full LLM-loop wired"
    );
    assert!(
        report.rows.iter().any(|row| {
            row.verb == "entity-workstream.update-status"
                && !row.core_acp_covered
                && row
                    .missing
                    .iter()
                    .any(|missing| missing == "acp_state_anchor_provider")
        }),
        "non-KYC/non-deal update-status surfaces should remain visible as ACP gaps"
    );

    write_acp_dsl_dag_coverage_artifacts(&report, output_dir())?;
    Ok(())
}

#[test]
#[ignore = "writes the human-readable ACP DSL/DAG coverage ledger"]
fn acp_dsl_dag_coverage_report_writes_artifacts() -> Result<()> {
    let report = build_acp_dsl_dag_coverage_report(config_root())?;
    let (json_path, md_path) = write_acp_dsl_dag_coverage_artifacts(&report, output_dir())?;
    println!("wrote {}", json_path.display());
    println!("wrote {}", md_path.display());
    Ok(())
}

#[test]
#[ignore = "100% ACP full-loop target; enable while burning down the coverage ledger"]
fn acp_dsl_dag_full_loop_coverage_target_is_100_percent() -> Result<()> {
    let report = build_acp_dsl_dag_coverage_report(config_root())?;
    if report.summary.full_loop_uncovered_rows != 0 {
        let (json_path, md_path) = write_acp_dsl_dag_coverage_artifacts(&report, output_dir())?;
        let first_gap = report
            .rows
            .iter()
            .find(|row| !row.full_loop_covered)
            .map(|row| format!("{} {} missing {:?}", row.source_file, row.verb, row.missing))
            .unwrap_or_else(|| "unknown gap".to_string());
        bail!(
            "ACP full-loop coverage is not 100% yet: {}/{} covered. First gap: {}. Reports: {}, {}",
            report.summary.full_loop_covered_rows,
            report.summary.total_rows,
            first_gap,
            json_path.display(),
            md_path.display()
        );
    }
    Ok(())
}
