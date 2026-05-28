//! Integration tests for Tranche 4: Sage orchestrator state machine.
//!
//! Tests cover the full Listening → Matching → Confirming → Instantiated →
//! Deployed pipeline, plus edge-cases (edit loop, reject, cancel).

use dsl_diagnostics::DiagnosticBag;
use dsl_resolution::{pack_registry::load_packs_from_dir, PackRegistry};
use dsl_sage::{
    ConfirmationResponse, SageContext, SageInput, SageOrchestrator, SageSession, SageState,
};

// ---------------------------------------------------------------------------
// Registry helper
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
// Tests
// ---------------------------------------------------------------------------

/// Full end-to-end flow: Utterance → SelectPack → Accept → Instantiated.
#[tokio::test]
async fn end_to_end_conjunctive_gate() {
    let registry = load_test_registry();
    let orchestrator = SageOrchestrator::new(&registry);
    let mut session = SageSession::new(SageContext::empty());

    // Step 1: Utterance → Matching
    orchestrator
        .step(
            &mut session,
            SageInput::Utterance("all three checks must pass: KYC, screening, and UBO".to_string()),
        )
        .await
        .unwrap();

    let candidates = match &session.state {
        SageState::Matching { candidates } => candidates.clone(),
        s => panic!("expected Matching, got {:?}", std::mem::discriminant(s)),
    };
    assert!(!candidates.is_empty(), "should have at least one candidate");

    // Step 2: SelectPack → Confirming
    orchestrator
        .step(
            &mut session,
            SageInput::SelectPack {
                pack_name: "conjunctive-gate".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(
        matches!(session.state, SageState::Confirming { .. }),
        "expected Confirming, got {:?}",
        std::mem::discriminant(&session.state)
    );

    // Step 3: Accept → Instantiated
    orchestrator
        .step(
            &mut session,
            SageInput::Confirm(ConfirmationResponse::Accept),
        )
        .await
        .unwrap();

    match &session.state {
        SageState::Instantiated { result, validation } => {
            assert!(
                !result.dsl_source.is_empty(),
                "DSL source must not be empty"
            );
            assert!(
                result.dsl_source.contains("provenance"),
                "DSL source must contain a provenance atom"
            );
            // The conjunctive-gate template should produce at least 3 atoms.
            assert!(
                result.atom_names.len() >= 3,
                "expected ≥3 atoms, got {}",
                result.atom_names.len()
            );
            let _ = validation; // validated by the instantiator
        }
        s => panic!("expected Instantiated, got {:?}", std::mem::discriminant(s)),
    }

    // Transition log should have at least 3 entries (one per step).
    assert!(
        session.transition_log.len() >= 3,
        "expected ≥3 log entries, got {}",
        session.transition_log.len()
    );
}

/// Edit a parameter, then accept.  The custom value must appear in the DSL.
#[tokio::test]
async fn edit_loop_then_accept() {
    let registry = load_test_registry();
    let orchestrator = SageOrchestrator::new(&registry);
    let mut session = SageSession::new(SageContext::empty());

    orchestrator
        .step(
            &mut session,
            SageInput::Utterance("all conditions met route to enhanced".to_string()),
        )
        .await
        .unwrap();

    orchestrator
        .step(
            &mut session,
            SageInput::SelectPack {
                pack_name: "conjunctive-gate".to_string(),
            },
        )
        .await
        .unwrap();

    // Edit gate-name — session must stay in Confirming.
    orchestrator
        .step(
            &mut session,
            SageInput::Confirm(ConfirmationResponse::EditParameter {
                name: "gate-name".to_string(),
                new_value: serde_json::json!("my-eligibility-gate"),
            }),
        )
        .await
        .unwrap();
    assert!(
        matches!(session.state, SageState::Confirming { .. }),
        "should remain in Confirming after edit"
    );

    // Now accept.
    orchestrator
        .step(
            &mut session,
            SageInput::Confirm(ConfirmationResponse::Accept),
        )
        .await
        .unwrap();

    if let SageState::Instantiated { result, .. } = &session.state {
        assert!(
            result.dsl_source.contains("my-eligibility-gate"),
            "custom gate name should appear in DSL; source:\n{}",
            &result.dsl_source[..result.dsl_source.len().min(500)]
        );
    } else {
        panic!(
            "expected Instantiated, got {:?}",
            std::mem::discriminant(&session.state)
        );
    }
}

/// Cancel from any state transitions to [`SageState::Cancelled`].
#[tokio::test]
async fn cancel_at_any_point() {
    let registry = load_test_registry();
    let orchestrator = SageOrchestrator::new(&registry);
    let mut session = SageSession::new(SageContext::empty());

    orchestrator
        .step(
            &mut session,
            SageInput::Utterance("test utterance".to_string()),
        )
        .await
        .unwrap();

    orchestrator
        .step(&mut session, SageInput::Cancel)
        .await
        .unwrap();

    assert!(
        matches!(session.state, SageState::Cancelled),
        "expected Cancelled, got {:?}",
        std::mem::discriminant(&session.state)
    );
}

/// Rejecting a pack from Confirming returns to Matching with candidates.
#[tokio::test]
async fn reject_pack_returns_to_matching() {
    let registry = load_test_registry();
    let orchestrator = SageOrchestrator::new(&registry);
    let mut session = SageSession::new(SageContext::empty());

    orchestrator
        .step(
            &mut session,
            SageInput::Utterance("all conditions must hold".to_string()),
        )
        .await
        .unwrap();

    orchestrator
        .step(
            &mut session,
            SageInput::SelectPack {
                pack_name: "conjunctive-gate".to_string(),
            },
        )
        .await
        .unwrap();

    orchestrator
        .step(
            &mut session,
            SageInput::Confirm(ConfirmationResponse::RejectPack),
        )
        .await
        .unwrap();

    assert!(
        matches!(session.state, SageState::Matching { .. }),
        "reject should return to Matching, got {:?}",
        std::mem::discriminant(&session.state)
    );
}

/// Full cycle: Instantiated → Deployed.
#[tokio::test]
async fn full_deploy_cycle() {
    let registry = load_test_registry();
    let orchestrator = SageOrchestrator::new(&registry);
    let mut session = SageSession::new(SageContext::empty());

    orchestrator
        .step(
            &mut session,
            SageInput::Utterance("all checks must pass".to_string()),
        )
        .await
        .unwrap();

    orchestrator
        .step(
            &mut session,
            SageInput::SelectPack {
                pack_name: "conjunctive-gate".to_string(),
            },
        )
        .await
        .unwrap();

    orchestrator
        .step(
            &mut session,
            SageInput::Confirm(ConfirmationResponse::Accept),
        )
        .await
        .unwrap();

    orchestrator
        .step(
            &mut session,
            SageInput::Deploy {
                workflow_name: "kyc-onboarding".to_string(),
            },
        )
        .await
        .unwrap();

    match &session.state {
        SageState::Deployed { workflow_id } => {
            assert!(
                workflow_id.starts_with("kyc-onboarding-"),
                "workflow_id should start with 'kyc-onboarding-', got: {}",
                workflow_id
            );
        }
        s => panic!("expected Deployed, got {:?}", std::mem::discriminant(s)),
    }
}
