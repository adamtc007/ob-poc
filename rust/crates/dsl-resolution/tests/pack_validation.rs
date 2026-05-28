//! Integration tests for dsl-resolution: pack loading, validation, and
//! provenance tracking.

use dsl_diagnostics::DiagnosticBag;
use dsl_resolution::pack_registry::load_packs_from_dir;
use dsl_resolution::{validate_bpmn, PackRegistry};

// ---------------------------------------------------------------------------
// Test 1: conjunctive-gate parses and validates cleanly
// ---------------------------------------------------------------------------

#[test]
fn conjunctive_gate_parses_and_validates() {
    let source = include_str!("../../../dsl-source/packs/conjunctive-gate.dsl");

    let (sf, parse_diag) = dsl_parser::parse(source);
    assert!(
        !parse_diag.has_errors(),
        "parse errors: {:?}",
        parse_diag.diagnostics
    );

    let mut diag = DiagnosticBag::new();
    // Merge parse diag
    for d in parse_diag.diagnostics {
        diag.push(d);
    }
    let bag = dsl_ast::AtomBag::from_source_file(sf, &mut diag);

    let mut registry = PackRegistry::new();
    dsl_resolution::resolve(&bag, &mut registry, &mut diag);

    assert!(
        !diag.has_errors(),
        "resolution errors: {:?}",
        diag.diagnostics
    );
    assert!(
        registry.lookup("conjunctive-gate", "1.0.0").is_some(),
        "expected 'conjunctive-gate' in registry"
    );
}

// ---------------------------------------------------------------------------
// Test 2: all 12 packs load without errors
// ---------------------------------------------------------------------------

#[test]
fn all_12_packs_load() {
    let packs_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/dsl-resolution -> crates/
        .unwrap()
        .parent() // crates/ -> rust/
        .unwrap()
        .join("dsl-source/packs");

    let mut registry = PackRegistry::new();
    let mut diag = DiagnosticBag::new();

    load_packs_from_dir(&packs_dir, &mut registry, &mut diag).expect("failed to read packs dir");

    assert!(
        !diag.has_errors(),
        "errors loading packs: {:?}",
        diag.diagnostics
    );
    assert_eq!(
        registry.len(),
        12,
        "expected 12 packs, got {}",
        registry.len()
    );
}

// ---------------------------------------------------------------------------
// Test 3: example-12 — pack-authored process validates end-to-end
//
// The expanded DSL is the conjunctive-gate instantiation (from design doc §9
// Example 12) embedded in a minimal complete process (start + end events added
// so that the BPMN assembly pass can find a valid start event).
// ---------------------------------------------------------------------------

// Example 12 DSL: a complete minimal process wrapping the conjunctive-gate
// instantiation from design doc §9. Condition expressions with nested atoms
// using non-symbol kinds (=, and, <) are stored as string literals because
// the v0.1 parser requires atom kinds to be unquoted symbols — operator
// characters like '=' are lex errors in that position.
const EXAMPLE_12_DSL: &str = r#"
(node onboarding-start :kind start-event)
(node pre-activation-check :kind user-task)
(node activate-cbu-task :kind service-task)
(node compliance-review-task :kind user-task)
(node end-activate :kind end-event)
(node end-review   :kind end-event)

(flow onboarding-start      -> pre-activation-check)
(flow activate-cbu-task     -> end-activate)
(flow compliance-review-task -> end-review)

(gateway activation-eligibility-gate :kind exclusive)
(flow pre-activation-check -> activation-eligibility-gate)
(flow activation-eligibility-gate -> activate-cbu-task :default false)
(flow activation-eligibility-gate -> compliance-review-task :default true)

; Note: provenance :covers uses kebab-case identifiers (no -> arrows) since
; the DSL symbol lexer uses [a-zA-Z0-9_-] only; -> is a reserved token.
(provenance activation-eligibility-gate-prov
  :covers [activation-eligibility-gate
           pre-check-to-gate-flow
           gate-to-activate-flow
           gate-to-review-flow]
  :source       pack
  :source-id    conjunctive-gate
  :version      "1.0.0"
  :session      "sess-019e4a1f-3b22-7e01-9f01-23f456789abc"
  :authored-at  "2026-05-21T12:00:00Z"
  :confirmed-at "2026-05-21T12:00:28Z"
  :params {
    :gate-name      activation-eligibility-gate
    :conditions     ["(= kyc-case.status approved)"
                     "(= ubo-status resolved)"
                     "(= sanctions-result clear)"]
    :enhanced-path  activate-cbu-task
    :standard-path  compliance-review-task
  })
"#;

#[test]
fn example_12_validates_end_to_end() {
    // Load the conjunctive-gate pack into the registry first.
    let pack_source = include_str!("../../../dsl-source/packs/conjunctive-gate.dsl");
    let (sf, _) = dsl_parser::parse(pack_source);
    let mut diag = DiagnosticBag::new();
    let bag = dsl_ast::AtomBag::from_source_file(sf, &mut diag);
    let mut registry = PackRegistry::new();
    dsl_resolution::resolve(&bag, &mut registry, &mut diag);
    assert!(registry.lookup("conjunctive-gate", "1.0.0").is_some());

    // Now validate Example 12.
    let response = validate_bpmn(EXAMPLE_12_DSL, "example-12", &mut registry);

    assert!(
        !response.has_errors,
        "expected no errors; got: {:?}",
        response.diagnostics
    );

    // Check provenance summary.
    assert_eq!(
        response.provenance_summary.instantiations.len(),
        1,
        "expected 1 provenance instantiation"
    );
    let inst = &response.provenance_summary.instantiations[0];
    assert_eq!(inst.pack_id, "conjunctive-gate");
    assert!(
        inst.covered_atoms.len() >= 3,
        "expected >= 3 covered atoms, got {}",
        inst.covered_atoms.len()
    );
}

// ---------------------------------------------------------------------------
// Test 4: template substitution forms outside :template slot
//
// In the v0.1 DSL, ,param-name forms (TemplateSubst) are only valid inside
// the :template slot of a (decision-pack ...) atom. When the assembly pass
// encounters a TemplateSubst in a flow :condition slot (for a flow that is not
// inside a template), it is an unresolved name-ref in the compiled form.
//
// Note: The parser accepts TemplateSubst tokens in any value position. Scope
// enforcement is therefore a resolution/assembly concern, not a parse concern.
// For v0.1, we verify that a stand-alone (outside-pack) TemplateSubst in a
// structural atom position does NOT cause a crash, and we document that full
// scope enforcement is deferred.
//
// TODO: scope enforcement via resolution pass (Tranche 6+)
// ---------------------------------------------------------------------------

#[test]
fn template_subst_in_gateway_does_not_panic() {
    // A TemplateSubst form in a :kind slot is unusual but the parser accepts it.
    // We verify the pipeline handles it gracefully (no panic, just a diagnostic).
    let source = r#"
(gateway ,param-name :kind exclusive)
"#;
    let (sf, parse_diag) = dsl_parser::parse(source);
    let mut diag = DiagnosticBag::new();
    for d in parse_diag.diagnostics {
        diag.push(d);
    }
    let bag = dsl_ast::AtomBag::from_source_file(sf, &mut diag);
    let mut registry = PackRegistry::new();
    // Should not panic.
    dsl_resolution::resolve(&bag, &mut registry, &mut diag);
    // The gateway will be classified as Structural(Gateway) regardless of the
    // TemplateSubst in the name position; no crash expected.
}

// ---------------------------------------------------------------------------
// Tests 5–16: Individual per-pack load and validate (9.3 sub-phase)
//
// Each test loads a single pack, resolves it, and asserts that the named pack
// is present in the registry after resolution.  Named individually so that a
// failure in one pack is immediately identifiable.
// ---------------------------------------------------------------------------

fn load_single_pack(filename: &str) -> PackRegistry {
    let packs_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("dsl-source/packs");
    let path = packs_dir.join(filename);
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {filename}: {e}"));
    let (sf, parse_diag) = dsl_parser::parse(&source);
    let mut diag = DiagnosticBag::new();
    for d in parse_diag.diagnostics {
        diag.push(d);
    }
    let bag = dsl_ast::AtomBag::from_source_file(sf, &mut diag);
    let mut registry = PackRegistry::new();
    dsl_resolution::resolve(&bag, &mut registry, &mut diag);
    assert!(
        !diag.has_errors(),
        "{filename} resolution errors: {:?}",
        diag.diagnostics
    );
    registry
}

#[test]
fn pack_cascading_decision_loads() {
    let r = load_single_pack("cascading-decision.dsl");
    assert!(
        r.lookup("cascading-decision", "1.0.0").is_some(),
        "cascading-decision not found in registry"
    );
}

#[test]
fn pack_conjunctive_gate_loads_individual() {
    let r = load_single_pack("conjunctive-gate.dsl");
    assert!(r.lookup("conjunctive-gate", "1.0.0").is_some());
}

#[test]
fn pack_decision_table_classification_loads() {
    let r = load_single_pack("decision-table-classification.dsl");
    assert!(r.lookup("decision-table-classification", "1.0.0").is_some());
}

#[test]
fn pack_disjunctive_gate_loads() {
    let r = load_single_pack("disjunctive-gate.dsl");
    assert!(r.lookup("disjunctive-gate", "1.0.0").is_some());
}

#[test]
fn pack_linked_switch_chain_loads() {
    let r = load_single_pack("linked-switch-chain.dsl");
    assert!(r.lookup("linked-switch-chain", "1.0.0").is_some());
}

#[test]
fn pack_manual_override_checkpoint_loads() {
    let r = load_single_pack("manual-override-checkpoint.dsl");
    assert!(r.lookup("manual-override-checkpoint", "1.0.0").is_some());
}

#[test]
fn pack_multi_jurisdiction_overlay_loads() {
    let r = load_single_pack("multi-jurisdiction-overlay.dsl");
    assert!(r.lookup("multi-jurisdiction-overlay", "1.0.0").is_some());
}

#[test]
fn pack_parallel_evaluation_with_veto_loads() {
    let r = load_single_pack("parallel-evaluation-with-veto.dsl");
    assert!(r.lookup("parallel-evaluation-with-veto", "1.0.0").is_some());
}

#[test]
fn pack_periodic_refresh_trigger_loads() {
    let r = load_single_pack("periodic-refresh-trigger.dsl");
    assert!(r.lookup("periodic-refresh-trigger", "1.0.0").is_some());
}

#[test]
fn pack_required_evidence_checklist_loads() {
    let r = load_single_pack("required-evidence-checklist.dsl");
    assert!(r.lookup("required-evidence-checklist", "1.0.0").is_some());
}

#[test]
fn pack_sanction_hit_escalation_loads() {
    let r = load_single_pack("sanction-hit-escalation.dsl");
    assert!(r.lookup("sanction-hit-escalation", "1.0.0").is_some());
}

#[test]
fn pack_threshold_band_routing_loads() {
    let r = load_single_pack("threshold-band-routing.dsl");
    assert!(r.lookup("threshold-band-routing", "1.0.0").is_some());
}

// ---------------------------------------------------------------------------
// Test 17: Registry queries (9.3 sub-phase)
// ---------------------------------------------------------------------------

#[test]
fn pack_registry_queries() {
    let packs_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("dsl-source/packs");

    let mut registry = PackRegistry::new();
    let mut diag = DiagnosticBag::new();

    load_packs_from_dir(&packs_dir, &mut registry, &mut diag).expect("failed to read packs dir");
    assert!(!diag.has_errors(), "errors: {:?}", diag.diagnostics);

    // Exact lookup
    assert!(
        registry.lookup("conjunctive-gate", "1.0.0").is_some(),
        "expected conjunctive-gate 1.0.0 in registry"
    );

    // Latest lookup
    assert!(
        registry.lookup_latest("periodic-refresh-trigger").is_some(),
        "expected periodic-refresh-trigger in registry"
    );

    // All 12 active
    assert_eq!(registry.list_active().len(), 12, "expected 12 active packs");

    // All packs have non-empty domain_scope
    for pack in registry.list_active() {
        assert!(
            !pack.domain_scope.is_empty(),
            "pack '{}' has empty domain_scope",
            pack.name
        );
    }

    // All packs have non-empty example_utterances
    for pack in registry.list_active() {
        assert!(
            !pack.example_utterances.is_empty(),
            "pack '{}' has empty example_utterances",
            pack.name
        );
    }
}

// ---------------------------------------------------------------------------
// Test 18: Pack DSL files all compile (9.3 sub-phase)
//
// Each pack's DSL source file is piped through the full resolution pipeline
// (parse -> bag -> resolve -> assemble) and verified error-free.
// ---------------------------------------------------------------------------

fn pack_source(pack_name: &str) -> &'static str {
    match pack_name {
        "conjunctive-gate" => include_str!("../../../dsl-source/packs/conjunctive-gate.dsl"),
        "disjunctive-gate" => include_str!("../../../dsl-source/packs/disjunctive-gate.dsl"),
        "sanction-hit-escalation" => {
            include_str!("../../../dsl-source/packs/sanction-hit-escalation.dsl")
        }
        "periodic-refresh-trigger" => {
            include_str!("../../../dsl-source/packs/periodic-refresh-trigger.dsl")
        }
        "manual-override-checkpoint" => {
            include_str!("../../../dsl-source/packs/manual-override-checkpoint.dsl")
        }
        "threshold-band-routing" => {
            include_str!("../../../dsl-source/packs/threshold-band-routing.dsl")
        }
        "multi-jurisdiction-overlay" => {
            include_str!("../../../dsl-source/packs/multi-jurisdiction-overlay.dsl")
        }
        "linked-switch-chain" => include_str!("../../../dsl-source/packs/linked-switch-chain.dsl"),
        "parallel-evaluation-with-veto" => {
            include_str!("../../../dsl-source/packs/parallel-evaluation-with-veto.dsl")
        }
        "cascading-decision" => include_str!("../../../dsl-source/packs/cascading-decision.dsl"),
        "decision-table-classification" => {
            include_str!("../../../dsl-source/packs/decision-table-classification.dsl")
        }
        "required-evidence-checklist" => {
            include_str!("../../../dsl-source/packs/required-evidence-checklist.dsl")
        }
        _ => panic!("unknown pack: {pack_name}"),
    }
}

#[test]
fn instantiate_all_packs_compiles() {
    // Pack definition files are declaration-only — they contain (decision-pack ...)
    // and (governance-status ...) atoms but no complete BPMN process.  The
    // relevant compile check is therefore parse + resolve (no assembly errors).
    let pack_names = [
        "conjunctive-gate",
        "disjunctive-gate",
        "sanction-hit-escalation",
        "periodic-refresh-trigger",
        "manual-override-checkpoint",
        "threshold-band-routing",
        "multi-jurisdiction-overlay",
        "linked-switch-chain",
        "parallel-evaluation-with-veto",
        "cascading-decision",
        "decision-table-classification",
        "required-evidence-checklist",
    ];

    for pack_name in &pack_names {
        let source = pack_source(pack_name);
        let (sf, parse_diag) = dsl_parser::parse(source);
        let mut diag = DiagnosticBag::new();
        for d in parse_diag.diagnostics {
            diag.push(d);
        }
        let bag = dsl_ast::AtomBag::from_source_file(sf, &mut diag);
        let mut registry = PackRegistry::new();
        dsl_resolution::resolve(&bag, &mut registry, &mut diag);
        assert!(
            !diag.has_errors(),
            "pack '{}' parse+resolve errors: {:?}",
            pack_name,
            diag.diagnostics
        );
        assert!(
            registry.lookup(pack_name, "1.0.0").is_some(),
            "pack '{}' not found in registry after resolve",
            pack_name
        );
    }
}
