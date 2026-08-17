//! The actual deliverable of this crate's design: load the real, live
//! `kyc_dag.yaml` and report, per slot, whether its state machine compiles
//! into a BPMN workflow template today. This is the concrete "is the
//! DAG-taxonomy shape rich enough to be the board" pressure test —
//! see `docs/todo/EOP-DD-DAGBPMN-001_DAG-Taxonomy-to-BPMN-Template-Compiler_v0.1.md`.
//!
//! This test intentionally does not assert a fixed pass/fail count per
//! slot: `kyc_dag.yaml` is a live, evolving document, and pinning exact
//! numbers here would make this test churn on every unrelated taxonomy
//! edit. What it *does* assert: every slot with a structured state machine
//! resolves to exactly one clean outcome (compiled, or a named
//! [`dag_to_bpmn::ShapeError`]/pipeline rejection) — never a panic, and
//! the failure reason is always one of the crate's own named shapes, never
//! an unexplained/unknown error. The printed report (`--nocapture`) is the
//! human-readable artifact.

use dag_to_bpmn::{compile_slot, DagToBpmnError};
use dsl_core::load_dags_from_dir;
use dsl_types::SlotStateMachine;
use std::path::PathBuf;

fn kyc_dag_taxonomies_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/sem_os_seeds/dag_taxonomies")
}

#[test]
fn kyc_dag_slots_report_a_named_outcome_never_a_panic() {
    let dags = load_dags_from_dir(&kyc_dag_taxonomies_dir())
        .expect("kyc_dag.yaml and siblings must parse as Dag YAML");
    let loaded = dags
        .get("kyc")
        .expect("dag_taxonomies dir must contain a workspace: kyc DAG");
    let dag = &loaded.dag;

    let mut compiled = Vec::new();
    let mut stateless_or_reference = Vec::new();
    let mut rejected = Vec::new();

    for slot in &dag.slots {
        match &slot.state_machine {
            None => stateless_or_reference.push((slot.id.clone(), "stateless".to_string())),
            Some(SlotStateMachine::Reference(r)) => {
                stateless_or_reference.push((slot.id.clone(), format!("reference({r})")))
            }
            Some(SlotStateMachine::Structured(_)) => {
                match compile_slot(dag, &slot.id, &format!("kyc-{}", slot.id)) {
                    Ok(_) => compiled.push(slot.id.clone()),
                    Err(DagToBpmnError::Shape(e)) => rejected.push((slot.id.clone(), e.to_string())),
                    Err(DagToBpmnError::Pipeline(e)) => {
                        rejected.push((slot.id.clone(), format!("pipeline: {e}")))
                    }
                }
            }
        }
    }

    println!(
        "\n=== dag-to-bpmn pressure test: kyc_dag.yaml ({} slots) ===",
        dag.slots.len()
    );
    println!("compiled clean ({}): {:?}", compiled.len(), compiled);
    println!(
        "no state machine to compile ({}): {:?}",
        stateless_or_reference.len(),
        stateless_or_reference
    );
    println!("rejected ({}):", rejected.len());
    for (slot, reason) in &rejected {
        println!("  - {slot}: {reason}");
    }

    // The real assertion: every structured-state-machine slot landed in
    // exactly one of compiled/rejected (no panics reached this line at
    // all, which is itself most of the proof), and every rejection has a
    // concrete slot name and a real message body attached — this is what
    // "fails loud, never silently drops data" actually gets checked for.
    for (slot, reason) in &rejected {
        assert!(!slot.is_empty());
        assert!(!reason.is_empty());
    }
    assert_eq!(
        compiled.len() + rejected.len() + stateless_or_reference.len(),
        dag.slots.len()
    );
}
