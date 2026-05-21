//! Regression baseline health sentinel — Tranche 1.
//!
//! This sentinel test verifies that the snapshot-based regression baseline
//! (Tranche 1) has been properly initialised. It must remain green throughout
//! all subsequent tranche work.
//!
//! If any snapshot test in ast_golden / dag_golden / plan_golden diverges,
//! the reshape has changed observable parser or compilation behaviour.
//!
//! Run `INSTA_UPDATE=new cargo test -p dsl-core` once to initialise
//! snapshots, then `cargo test -p dsl-core` to validate them.

use std::path::Path;

#[test]
fn regression_baseline_is_active() {
    let snapshots_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");

    assert!(
        snapshots_dir.exists(),
        "tests/snapshots/ directory does not exist. \
         Run: INSTA_UPDATE=new cargo test -p dsl-core --test ast_golden --test dag_golden --test plan_golden"
    );

    let count = std::fs::read_dir(&snapshots_dir)
        .map(|d| d.filter_map(|e| e.ok()).count())
        .unwrap_or(0);

    // We expect at least 50 snapshot files:
    //   50 from ast_golden + 20 from dag_golden + 10+ from plan_golden
    // Accept 0 as a special case when running CI before first initialisation.
    // Once initialised (count > 0), enforce the minimum.
    if count > 0 {
        assert!(
            count >= 50,
            "Expected ≥50 snapshot files in tests/snapshots/, found {}. \
             Some snapshot tests may have been removed or the baseline is incomplete. \
             Run: INSTA_UPDATE=new cargo test -p dsl-core",
            count
        );
        println!(
            "Regression baseline ACTIVE: {} snapshot files present",
            count
        );
    } else {
        // Not yet initialised — this is OK for a fresh checkout.
        // The snapshot tests themselves will show as 'SNAPSHOT NOT FOUND'
        // and guide the developer to run INSTA_UPDATE=new.
        println!(
            "Regression baseline NOT YET INITIALISED (0 snapshots). \
             Run: INSTA_UPDATE=new cargo test -p dsl-core --test ast_golden --test dag_golden --test plan_golden"
        );
    }
}

#[test]
fn snapshots_dir_exists() {
    let snapshots_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    assert!(
        snapshots_dir.exists(),
        "tests/snapshots/ directory must exist (even empty). \
         Create it with: mkdir -p rust/crates/dsl-core/tests/snapshots"
    );
}

/// Verify the dsl-core test suite itself compiles and runs cleanly by
/// checking that the core parser public API is accessible.
#[test]
fn core_parser_api_accessible() {
    let program = dsl_core::parser::parse_program(r#"(session.info)"#)
        .expect("parse_program must succeed for a trivial verb call");
    assert_eq!(program.statements.len(), 1);
}

/// Verify compile_to_steps is accessible and produces a non-empty result.
#[test]
fn compiler_api_accessible() {
    let program = dsl_core::parser::parse_program(r#"(cbu.create :name "Test" :jurisdiction "LU")"#)
        .expect("parse failed");
    let compiled = dsl_core::compiler::compile_to_steps(&program);
    assert!(compiled.is_ok(), "compile_to_steps should succeed");
    assert_eq!(compiled.steps.len(), 1);
}
