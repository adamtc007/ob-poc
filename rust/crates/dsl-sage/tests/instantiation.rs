//! Integration tests for Tranche 3: pack instantiation.
//!
//! Tests cover:
//! - End-to-end instantiation producing well-formed DSL + provenance atom.
//! - Compile validation through the v0.1 pipeline.
//! - Provenance atom listing the correct atom names.
//! - All 12 seed packs instantiating without panic.

use std::collections::HashMap;

use dsl_diagnostics::DiagnosticBag;
use dsl_resolution::{pack_registry::load_packs_from_dir, PackRegistry};
use dsl_sage::{instantiate, validate_instantiation, SageContext};

// ---------------------------------------------------------------------------
// Registry helper (mirrors pack_matching_eval.rs)
// ---------------------------------------------------------------------------

fn load_test_registry() -> PackRegistry {
    let pack_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("dsl-source/packs");

    let mut registry = PackRegistry::new();
    let mut diag = DiagnosticBag::new();
    load_packs_from_dir(&pack_dir, &mut registry, &mut diag)
        .expect("failed to load pack DSL files");

    let errors: Vec<_> = diag
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, dsl_diagnostics::DiagnosticSeverity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "pack loading produced errors: {:?}",
        errors
    );
    assert!(
        registry.len() >= 12,
        "expected at least 12 packs, got {}",
        registry.len()
    );
    registry
}

// ---------------------------------------------------------------------------
// T3-1: conjunctive-gate instantiates and compiles
// ---------------------------------------------------------------------------

#[test]
fn conjunctive_gate_instantiates_and_compiles() {
    let registry = load_test_registry();
    let context = SageContext::empty();
    let params: HashMap<String, serde_json::Value> = [
        ("gate-name".to_string(), serde_json::json!("kyc-eligibility-gate")),
        ("enhanced-path".to_string(), serde_json::json!("activate-end")),
        ("standard-path".to_string(), serde_json::json!("review-end")),
    ]
    .into_iter()
    .collect();

    let result = instantiate(
        "conjunctive-gate",
        "1.0.0",
        &params,
        Some("kyc-complete"),
        &context,
        &registry,
    )
    .expect("instantiation failed");

    // Structural DSL is non-empty
    assert!(!result.structural_dsl.is_empty(), "structural_dsl should not be empty");

    // Full source includes provenance atom
    assert!(
        result.dsl_source.contains("provenance"),
        "dsl_source should contain provenance atom"
    );
    assert!(
        result.dsl_source.contains("conjunctive-gate"),
        "dsl_source should reference pack name"
    );

    // Atom names extracted
    assert!(
        !result.atom_names.is_empty(),
        "atom_names should not be empty"
    );
    assert!(
        result.atom_names.contains(&"kyc-eligibility-gate".to_string()),
        "gate name should be in atom_names, got: {:?}",
        result.atom_names
    );

    // Validate structural DSL compiles
    let summary = validate_instantiation(&result.structural_dsl)
        .expect("validate_instantiation failed");
    assert!(
        !summary.has_errors,
        "Compile errors for conjunctive-gate: {:?}",
        summary.diagnostics
    );
    assert!(summary.node_count > 0, "graph should have nodes");
}

// ---------------------------------------------------------------------------
// T3-2: all 12 packs instantiate without panic
// ---------------------------------------------------------------------------

#[test]
fn all_12_packs_instantiate_without_panic() {
    let registry = load_test_registry();
    let context = SageContext::empty();

    let test_cases: Vec<(&str, serde_json::Value)> = vec![
        (
            "conjunctive-gate",
            serde_json::json!({
                "gate-name": "cg-gate",
                "enhanced-path": "cg-enhanced",
                "standard-path": "cg-standard"
            }),
        ),
        (
            "disjunctive-gate",
            serde_json::json!({
                "gate-name": "dg-gate",
                "escalation-path": "dg-escalation",
                "standard-path": "dg-standard"
            }),
        ),
        (
            "sanction-hit-escalation",
            serde_json::json!({
                "sanctions-check-name": "sc-check",
                "sanctions-gate-name": "sc-gate",
                "escalation-path": "sc-escalation",
                "clear-path": "sc-clear"
            }),
        ),
        (
            "periodic-refresh-trigger",
            serde_json::json!({
                "age-gate-name": "prt-gate",
                "refresh-path": "prt-refresh",
                "current-path": "prt-current"
            }),
        ),
        (
            "manual-override-checkpoint",
            serde_json::json!({
                "auto-eval-name": "moc-eval",
                "review-task-name": "moc-review",
                "override-gate-name": "moc-gate",
                "confirmed-path": "moc-confirmed",
                "override-path": "moc-override"
            }),
        ),
        (
            "threshold-band-routing",
            serde_json::json!({
                "band-gate-name": "tbr-gate",
                "bands": [
                    {"upper": 10, "path": "low-end"},
                    {"upper": 25, "path": "mid-end"},
                    {"path": "high-end"}
                ]
            }),
        ),
        (
            "multi-jurisdiction-overlay",
            serde_json::json!({
                "jur-gate-name": "jur-gate",
                "jurisdiction-paths": [{"code": "GB", "path": "uk-path"}],
                "default-path": "global-path"
            }),
        ),
        (
            "linked-switch-chain",
            serde_json::json!({
                "gateway-names": [
                    {"name": "g1", "exit-path": "rej1"},
                    {"name": "g2", "exit-path": "rej2"}
                ],
                "final-path": "final-end"
            }),
        ),
        (
            "cascading-decision",
            serde_json::json!({
                "primary-eval-name": "cd-eval",
                "primary-gate-name": "cd-gate",
                "paths": [
                    {"value": "corporate", "path": "corp-end"},
                    {"value": "individual", "path": "ind-end"}
                ]
            }),
        ),
        (
            "decision-table-classification",
            serde_json::json!({
                "classify-name": "dtc-classify",
                "route-gate-name": "dtc-gate",
                "paths": [
                    {"value": "low", "path": "low-end"},
                    {"value": "high", "path": "high-end"}
                ]
            }),
        ),
        (
            "parallel-evaluation-with-veto",
            serde_json::json!({
                "fork-name": "pev-fork",
                "join-name": "pev-join",
                "post-join-gate": "pev-gate",
                "eval-tasks": [{"name": "task-a"}, {"name": "task-b"}],
                "vetoed-path": "vetoed-end",
                "approved-path": "approved-end"
            }),
        ),
        (
            "required-evidence-checklist",
            serde_json::json!({
                "tasks": [{"name": "task1"}, {"name": "task2"}],
                "checklist-gate-name": "rec-gate",
                "approval-path": "approved-end",
                "rejection-path": "rejected-end"
            }),
        ),
    ];

    for (pack_name, params_val) in &test_cases {
        let param_map: HashMap<String, serde_json::Value> = params_val
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let result = instantiate(pack_name, "1.0.0", &param_map, Some("start-node"), &context, &registry);
        assert!(
            result.is_ok(),
            "pack '{}' instantiation failed: {:?}",
            pack_name,
            result.err()
        );

        let result = result.unwrap();
        assert!(
            result.dsl_source.contains("provenance"),
            "pack '{}' missing provenance atom in dsl_source",
            pack_name
        );
        assert!(
            !result.structural_dsl.is_empty(),
            "pack '{}' produced empty structural_dsl",
            pack_name
        );
    }
}

// ---------------------------------------------------------------------------
// T3-3: provenance atom covers generated atoms
// ---------------------------------------------------------------------------

#[test]
fn provenance_atom_covers_generated_atoms() {
    let registry = load_test_registry();
    let context = SageContext::empty();
    let params: HashMap<String, serde_json::Value> = [
        ("gate-name".to_string(), serde_json::json!("test-gate")),
        ("enhanced-path".to_string(), serde_json::json!("enhanced-end")),
        ("standard-path".to_string(), serde_json::json!("standard-end")),
    ]
    .into_iter()
    .collect();

    let result =
        instantiate("conjunctive-gate", "1.0.0", &params, None, &context, &registry).unwrap();

    // The gate name should appear in the DSL
    assert!(
        result.dsl_source.contains("test-gate"),
        "gate name should appear in DSL"
    );

    // The gate atom should be tracked in atom_names
    assert!(
        result.atom_names.contains(&"test-gate".to_string()),
        "test-gate should be in atom_names, got: {:?}",
        result.atom_names
    );

    // The provenance atom should list the gate in :covers
    assert!(
        result.dsl_source.contains("test-gate"),
        "test-gate should appear in the provenance :covers list"
    );
}

// ---------------------------------------------------------------------------
// T3-4: unknown pack returns Err
// ---------------------------------------------------------------------------

#[test]
fn unknown_pack_returns_error() {
    let registry = load_test_registry();
    let context = SageContext::empty();
    let params: HashMap<String, serde_json::Value> = HashMap::new();

    let result = instantiate("nonexistent-pack", "1.0.0", &params, None, &context, &registry);
    assert!(result.is_err(), "expected Err for unknown pack");
}

// ---------------------------------------------------------------------------
// T3-5: all 12 structural DSLs compile without errors
// ---------------------------------------------------------------------------

#[test]
fn all_12_packs_structural_dsl_compile_clean() {
    let registry = load_test_registry();
    let context = SageContext::empty();

    let test_cases: Vec<(&str, serde_json::Value)> = vec![
        (
            "conjunctive-gate",
            serde_json::json!({"gate-name": "cg", "enhanced-path": "cg-e", "standard-path": "cg-s"}),
        ),
        (
            "disjunctive-gate",
            serde_json::json!({"gate-name": "dg", "escalation-path": "dg-e", "standard-path": "dg-s"}),
        ),
        (
            "sanction-hit-escalation",
            serde_json::json!({"sanctions-check-name": "sc", "sanctions-gate-name": "sg", "escalation-path": "se", "clear-path": "sc2"}),
        ),
        (
            "periodic-refresh-trigger",
            serde_json::json!({"age-gate-name": "ag", "refresh-path": "rp", "current-path": "cp"}),
        ),
        (
            "manual-override-checkpoint",
            serde_json::json!({"auto-eval-name": "ae", "review-task-name": "rt", "override-gate-name": "og", "confirmed-path": "cf", "override-path": "op"}),
        ),
        (
            "threshold-band-routing",
            serde_json::json!({"band-gate-name": "bg", "path-low": "bl", "path-mid": "bm", "path-high": "bh"}),
        ),
        (
            "multi-jurisdiction-overlay",
            serde_json::json!({"jur-gate-name": "jg", "path-a": "pa", "path-b": "pb", "default-path": "dp"}),
        ),
        (
            "linked-switch-chain",
            serde_json::json!({"gate-1-name": "g1", "gate-2-name": "g2", "exit-path-1": "e1", "exit-path-2": "e2", "final-path": "fp"}),
        ),
        (
            "cascading-decision",
            serde_json::json!({"primary-eval-name": "pe", "primary-gate-name": "pg", "path-a": "pa", "path-b": "pb"}),
        ),
        (
            "decision-table-classification",
            serde_json::json!({"classify-name": "cn", "route-gate-name": "rg", "path-a": "pa", "default-path": "dp2"}),
        ),
        (
            "parallel-evaluation-with-veto",
            serde_json::json!({"fork-name": "fk", "join-name": "jn", "post-join-gate": "pjg", "vetoed-path": "vp", "approved-path": "ap"}),
        ),
        (
            "required-evidence-checklist",
            serde_json::json!({"task-1": "t1", "task-2": "t2", "task-3": "t3", "checklist-gate-name": "cg2", "approval-path": "ap2", "rejection-path": "rp2"}),
        ),
    ];

    for (pack_name, params_val) in &test_cases {
        let param_map: HashMap<String, serde_json::Value> = params_val
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let result =
            instantiate(pack_name, "1.0.0", &param_map, None, &context, &registry).unwrap();

        let summary = validate_instantiation(&result.structural_dsl).unwrap();
        assert!(
            !summary.has_errors,
            "pack '{}' structural DSL has compile errors: {:?}",
            pack_name,
            summary.diagnostics
        );
        assert!(
            summary.node_count > 0,
            "pack '{}' graph has no nodes after compilation",
            pack_name
        );
    }
}
