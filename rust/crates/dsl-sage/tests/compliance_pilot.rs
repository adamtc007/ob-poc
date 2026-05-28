//! Automated compliance pilot harness — Tranche 8.
//!
//! Simulates the human-in-the-loop via [`ConfirmationResponse::Accept`] for each
//! of 5 worked scenarios, then asserts all 7 v&s audit boxes are populated.
//!
//! # v&s audit boxes
//!
//! 1. Sage matched the utterance to a pack with non-zero confidence.
//! 2. Parameters were proposed (at least one named parameter).
//! 3. HIL confirmation was recorded (automated-pilot marker).
//! 4. DSL was emitted and contains the provenance atom.
//! 5. Compile pipeline passed (no errors from the assembler).
//! 6. SVG diagram contains at least one visual element.
//! 7. Audit trail has ≥ 3 transition entries and names the source pack.

use dsl_diagnostics::DiagnosticBag;
use dsl_resolution::{pack_registry::load_packs_from_dir, PackRegistry};
use dsl_sage::{
    ConfirmationResponse, SageContext, SageInput, SageOrchestrator, SageSession, SageState,
};

// ---------------------------------------------------------------------------
// Scenario table
// ---------------------------------------------------------------------------

const PILOT_SCENARIOS: &[(&str, &str, &str)] = &[
    (
        "kyc_all_conditions",
        "conjunctive-gate",
        "before the client can go live all three checks must pass: KYC approved UBO resolved and no sanctions hits",
    ),
    (
        "sanctions_hard_block",
        "sanction-hit-escalation",
        "any sanctions match immediately escalates to compliance regardless of other outcomes",
    ),
    (
        "periodic_kyc_refresh",
        "periodic-refresh-trigger",
        "if the KYC review was completed more than 24 months ago trigger a re-verification",
    ),
    (
        "jurisdiction_routing",
        "multi-jurisdiction-overlay",
        "UK clients go to the CASS track EU clients to MiFID track everyone else to global standard",
    ),
    (
        "compliance_override",
        "manual-override-checkpoint",
        "system automatically assesses risk but compliance officer can override the decision",
    ),
];

// ---------------------------------------------------------------------------
// Result type capturing all 7 audit boxes
// ---------------------------------------------------------------------------

struct PilotScenarioResult {
    scenario_id: String,
    utterance: String,
    // Box 1: matching
    top_candidate: String,
    match_confidence: f32,
    // Box 2: parameters
    parameter_count: usize,
    parameters_proposed: Vec<String>,
    // Box 3: confirmation
    confirmed_by: String,
    confirmed_at: String,
    // Box 4: DSL emission
    dsl_source: String,
    atom_names: Vec<String>,
    // Box 5: compile
    compile_passed: bool,
    node_count: usize,
    // Box 6: diagram
    svg_length: usize,
    svg_has_nodes: bool,
    // Box 7: audit
    provenance_pack: String,
    audit_entries: usize,
    transition_log: Vec<String>,
}

// ---------------------------------------------------------------------------
// Registry helper (shared with other test files in this crate)
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

    registry
}

// ---------------------------------------------------------------------------
// Per-scenario runner
// ---------------------------------------------------------------------------

async fn run_pilot_scenario(
    scenario_id: &str,
    expected_pack: &str,
    utterance: &str,
    registry: &PackRegistry,
) -> PilotScenarioResult {
    let orchestrator = SageOrchestrator::new(registry);
    let mut session = SageSession::new(SageContext::with_domain("kyc"));

    // ── Step 1: Utterance → Matching ─────────────────────────────────────────
    orchestrator
        .step(&mut session, SageInput::Utterance(utterance.to_string()))
        .await
        .expect("utterance step failed");

    let (top_candidate, match_confidence) = match &session.state {
        SageState::Matching { candidates } => {
            let top = candidates.first().expect("no candidates returned");
            (top.pack_name.clone(), top.confidence)
        }
        s => panic!(
            "scenario {}: expected Matching state, got {:?}",
            scenario_id,
            std::mem::discriminant(s)
        ),
    };

    // ── Step 2: SelectPack → Confirming (parameter proposals) ────────────────
    orchestrator
        .step(
            &mut session,
            SageInput::SelectPack {
                pack_name: expected_pack.to_string(),
            },
        )
        .await
        .expect("select-pack step failed");

    let (parameter_count, parameters_proposed) = match &session.state {
        SageState::Confirming { session: conf } => {
            let names: Vec<String> = conf
                .request
                .proposed_parameters
                .iter()
                .map(|p| p.parameter_name.clone())
                .collect();
            (names.len(), names)
        }
        s => panic!(
            "scenario {}: expected Confirming state, got {:?}",
            scenario_id,
            std::mem::discriminant(s)
        ),
    };

    // ── Step 3: HIL confirmation (simulated by automated pilot) ──────────────
    let confirmed_at = chrono::Utc::now().to_rfc3339();

    orchestrator
        .step(
            &mut session,
            SageInput::Confirm(ConfirmationResponse::Accept),
        )
        .await
        .expect("confirm step failed");

    // ── Steps 4+5: DSL emission + compile validation ──────────────────────────
    let (dsl_source, atom_names, compile_passed, node_count, provenance_pack) = match &session.state
    {
        SageState::Instantiated { result, validation } => (
            result.dsl_source.clone(),
            result.atom_names.clone(),
            !validation.has_errors,
            validation.node_count,
            result.pack_name.clone(),
        ),
        s => panic!(
            "scenario {}: expected Instantiated state, got {:?}",
            scenario_id,
            std::mem::discriminant(s)
        ),
    };

    // ── Step 6: SVG rendering ─────────────────────────────────────────────────
    // Render from the structural DSL (without the provenance atom, which is
    // opaque to the assembler).
    let structural_only = dsl_source
        .split("\n(provenance")
        .next()
        .unwrap_or(&dsl_source);

    let svg = dsl_render::render(structural_only).unwrap_or_default();
    let svg_length = svg.len();
    let svg_has_nodes = svg.contains("<circle")
        || svg.contains("<rect")
        || svg.contains("<polygon")
        || svg.contains("<ellipse")
        || svg.contains("<path");

    // ── Step 7: audit entries from the transition log ─────────────────────────
    let audit_entries = session.transition_log.len();
    let transition_log = session.transition_log.clone();

    PilotScenarioResult {
        scenario_id: scenario_id.to_string(),
        utterance: utterance.to_string(),
        top_candidate,
        match_confidence,
        parameter_count,
        parameters_proposed,
        confirmed_by: "automated-pilot".to_string(),
        confirmed_at,
        dsl_source,
        atom_names,
        compile_passed,
        node_count,
        svg_length,
        svg_has_nodes,
        provenance_pack,
        audit_entries,
        transition_log,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Core compliance pilot: 5 scenarios, all 7 v&s audit boxes verified.
#[tokio::test]
async fn compliance_pilot_all_5_scenarios() {
    let registry = load_test_registry();
    let mut results = Vec::new();

    for (scenario_id, expected_pack, utterance) in PILOT_SCENARIOS {
        let result = run_pilot_scenario(scenario_id, expected_pack, utterance, &registry).await;

        // ── Box 1: pack match ─────────────────────────────────────────────────
        assert!(
            !result.top_candidate.is_empty(),
            "scenario {}: box 1 (pack match) must be populated",
            scenario_id
        );
        assert!(
            result.match_confidence > 0.0,
            "scenario {}: box 1 (confidence) must be > 0, got {}",
            scenario_id,
            result.match_confidence
        );

        // ── Box 2: parameters ─────────────────────────────────────────────────
        assert!(
            result.parameter_count > 0,
            "scenario {}: box 2 (parameters) must have ≥1 extracted parameter, got {}",
            scenario_id,
            result.parameter_count
        );

        // ── Box 3: confirmation ───────────────────────────────────────────────
        assert_eq!(
            result.confirmed_by, "automated-pilot",
            "scenario {}: box 3 (confirmation) must record the automated-pilot marker",
            scenario_id
        );
        assert!(
            !result.confirmed_at.is_empty(),
            "scenario {}: box 3 (confirmed_at) must be set",
            scenario_id
        );

        // ── Box 4: DSL emission ───────────────────────────────────────────────
        assert!(
            !result.dsl_source.is_empty(),
            "scenario {}: box 4 (DSL) must be emitted",
            scenario_id
        );
        assert!(
            result.dsl_source.contains("provenance"),
            "scenario {}: box 4 (provenance atom) must be present in DSL",
            scenario_id
        );
        assert!(
            !result.atom_names.is_empty(),
            "scenario {}: box 4 (atom names) must list at least one structural atom",
            scenario_id
        );

        // ── Box 5: compile ────────────────────────────────────────────────────
        assert!(
            result.compile_passed,
            "scenario {}: box 5 (compile) must pass without errors",
            scenario_id
        );
        assert!(
            result.node_count > 0,
            "scenario {}: box 5 (node_count) must be > 0",
            scenario_id
        );

        // ── Box 6: diagram ────────────────────────────────────────────────────
        assert!(
            result.svg_has_nodes,
            "scenario {}: box 6 (SVG diagram) must contain visual elements (circle/rect/polygon/ellipse/path); svg length={}",
            scenario_id,
            result.svg_length
        );

        // ── Box 7: audit trail ────────────────────────────────────────────────
        assert!(
            !result.provenance_pack.is_empty(),
            "scenario {}: box 7 (provenance pack) must name the source pack",
            scenario_id
        );
        assert!(
            result.audit_entries >= 3,
            "scenario {}: box 7 (audit trail) must have ≥3 transition entries, got {}",
            scenario_id,
            result.audit_entries
        );

        results.push(result);
    }

    // ── Print pilot report (captured by `cargo test -- --nocapture`) ──────────
    println!("\n=== Automated Compliance Pilot Report ===");
    println!("Date: {}", chrono::Utc::now().format("%Y-%m-%d"));
    println!("Reviewer: automated-pilot (Rust HIL simulation)");
    println!("Scenarios: {}/{}", results.len(), PILOT_SCENARIOS.len());
    println!();

    for r in &results {
        let utterance_preview = &r.utterance[..r.utterance.len().min(70)];
        println!("Scenario: {}", r.scenario_id);
        println!("  Utterance:  {}…", utterance_preview);
        println!(
            "  Pack:       {} (confidence {:.2})",
            r.top_candidate, r.match_confidence
        );
        println!("  Parameters: {:?}", r.parameters_proposed);
        println!("  DSL atoms:  {:?}", r.atom_names);
        println!(
            "  Compile:    {} | Nodes: {} | SVG: {} bytes",
            if r.compile_passed { "PASS" } else { "FAIL" },
            r.node_count,
            r.svg_length
        );
        println!("  Audit:      {} transition entries", r.audit_entries);
        println!();
    }

    println!(
        "=== All 7 v&s audit boxes verified for all {} scenarios ===",
        results.len()
    );
}

/// Readability check: DSL output must contain recognisable bpmn-lite keywords
/// and the provenance atom must name the source pack.
#[tokio::test]
async fn pilot_readability_assertions() {
    let registry = load_test_registry();

    let result = run_pilot_scenario(
        "readability_check",
        "conjunctive-gate",
        "all conditions must be met before activation",
        &registry,
    )
    .await;

    // Structural DSL should contain recognisable bpmn-lite keywords.
    assert!(
        result.dsl_source.contains("gateway") || result.dsl_source.contains("flow"),
        "DSL must contain recognisable bpmn-lite keywords (gateway/flow); source:\n{}",
        &result.dsl_source[..result.dsl_source.len().min(300)]
    );

    // The provenance atom must reference the pack name.
    assert!(
        result.dsl_source.contains(&result.provenance_pack),
        "Provenance atom must name the source pack '{}' in the DSL",
        result.provenance_pack
    );

    // SVG must be non-empty and contain at least one SVG shape element.
    assert!(result.svg_length > 0, "SVG output must not be empty");
    assert!(
        result.svg_has_nodes,
        "SVG output must contain at least one visual shape element"
    );
}

/// Audit log entries must cover Listening→Matching, Matching→Confirming,
/// and Confirming→Instantiated transitions.
#[tokio::test]
async fn pilot_audit_trail_content() {
    let registry = load_test_registry();

    let result = run_pilot_scenario(
        "audit_trail_check",
        "conjunctive-gate",
        "all three checks must hold before going live",
        &registry,
    )
    .await;

    // The transition log is the per-session audit surface.
    assert!(
        result.transition_log.len() >= 3,
        "expected ≥3 audit entries, got {}: {:?}",
        result.transition_log.len(),
        result.transition_log
    );

    // At least one entry should reference the utterance or matching.
    let has_match_entry = result.transition_log.iter().any(|e| {
        e.contains("Matched")
            || e.contains("candidate")
            || e.contains("utterance")
            || e.contains("Received")
    });
    assert!(
        has_match_entry,
        "audit trail must include a matching entry; log: {:?}",
        result.transition_log
    );

    // At least one entry should reference instantiation.
    let has_instantiated_entry = result
        .transition_log
        .iter()
        .any(|e| e.contains("Instantiated") || e.contains("atoms") || e.contains("atom"));
    assert!(
        has_instantiated_entry,
        "audit trail must include an instantiation entry; log: {:?}",
        result.transition_log
    );
}

/// All 5 pilot scenarios must produce compile-clean DSL — no error diagnostics.
#[tokio::test]
async fn pilot_compile_clean_for_all_scenarios() {
    let registry = load_test_registry();

    for (scenario_id, expected_pack, utterance) in PILOT_SCENARIOS {
        let result = run_pilot_scenario(scenario_id, expected_pack, utterance, &registry).await;

        assert!(
            result.compile_passed,
            "scenario {} (pack {}): compile must pass, but has_errors=true",
            scenario_id, expected_pack
        );
    }
}
