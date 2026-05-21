//! Integration tests for Tranche 2: parameter extraction and confirmation.

use dsl_diagnostics::DiagnosticBag;
use dsl_resolution::{pack_registry::load_packs_from_dir, PackRegistry};
use dsl_sage::{
    extract_parameters, ConfirmationRequest, ConfirmationResponse, ConfirmationSession,
    ConfirmationState, HeuristicExtractor, ParameterProposal, SageContext,
};

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
// Request builder for tests that do not need a registry
// ---------------------------------------------------------------------------

fn make_request() -> ConfirmationRequest {
    ConfirmationRequest {
        pack_name: "conjunctive-gate".to_string(),
        pack_version: "1.0.0".to_string(),
        proposed_parameters: vec![ParameterProposal {
            parameter_name: "gate-name".to_string(),
            proposed_value: serde_json::json!("initial-gate"),
            confidence: 0.8,
            rationale: "test".to_string(),
            source_phrase: None,
        }],
        preview_dsl: String::new(),
    }
}

// ---------------------------------------------------------------------------
// Parameter extraction tests
// ---------------------------------------------------------------------------

#[test]
fn heuristic_extracts_conjunctive_gate_params() {
    let registry = load_test_registry();
    let pack = registry.lookup("conjunctive-gate", "1.0.0").unwrap();

    let utterance = "all three checks must pass: KYC approved, screening clear, UBO resolved";
    let proposals = HeuristicExtractor::extract(utterance, pack);

    // Should have exactly as many proposals as parameters
    assert_eq!(
        proposals.len(),
        pack.parameters.len(),
        "proposal count does not match parameter count for conjunctive-gate"
    );
    // Every parameter must have a proposal
    for param in &pack.parameters {
        assert!(
            proposals.iter().any(|p| p.parameter_name == param.name),
            "no proposal for parameter '{}'",
            param.name
        );
    }
}

#[test]
fn all_12_packs_extract_without_panic() {
    let registry = load_test_registry();
    let utterance = "process this request using the appropriate workflow";
    for pack in registry.list_active() {
        let proposals = HeuristicExtractor::extract(utterance, pack);
        assert_eq!(
            proposals.len(),
            pack.parameters.len(),
            "pack '{}' parameter count mismatch: expected {} proposals, got {}",
            pack.name,
            pack.parameters.len(),
            proposals.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Confirmation session tests
// ---------------------------------------------------------------------------

#[test]
fn confirmation_session_accept_flow() {
    let request = ConfirmationRequest {
        pack_name: "conjunctive-gate".to_string(),
        pack_version: "1.0.0".to_string(),
        proposed_parameters: vec![ParameterProposal {
            parameter_name: "gate-name".to_string(),
            proposed_value: serde_json::Value::String("kyc-gate".to_string()),
            confidence: 0.9,
            rationale: "extracted".to_string(),
            source_phrase: None,
        }],
        preview_dsl: "(gateway kyc-gate :kind exclusive)".to_string(),
    };

    let mut session = ConfirmationSession::new(request);
    assert_eq!(session.state, ConfirmationState::Pending);

    let state = session.apply_response(ConfirmationResponse::Accept);
    assert_eq!(state, ConfirmationState::Accepted);
    assert!(session.confirmed_parameters().is_some());
}

#[test]
fn confirmation_edit_loop() {
    let mut session = ConfirmationSession::new(make_request());

    // Edit a parameter — should stay Pending
    session.apply_response(ConfirmationResponse::EditParameter {
        name: "gate-name".to_string(),
        new_value: serde_json::Value::String("my-custom-gate".to_string()),
    });
    assert_eq!(
        session.state,
        ConfirmationState::Pending,
        "should stay pending after edit"
    );

    // Accept after edit
    session.apply_response(ConfirmationResponse::Accept);

    let params = session.confirmed_parameters().unwrap();
    assert_eq!(
        params["gate-name"],
        serde_json::Value::String("my-custom-gate".to_string())
    );
    assert_eq!(session.edit_history.len(), 1);
}

#[test]
fn confirmation_reject_returns_to_matching() {
    let mut session = ConfirmationSession::new(make_request());
    let state = session.apply_response(ConfirmationResponse::RejectPack);
    assert_eq!(state, ConfirmationState::Rejected);
    assert!(session.confirmed_parameters().is_none());
}

#[test]
fn confirmation_cancel_is_terminal() {
    let mut session = ConfirmationSession::new(make_request());
    let state = session.apply_response(ConfirmationResponse::Cancel);
    assert_eq!(state, ConfirmationState::Cancelled);
    assert!(session.is_terminal());
    assert!(session.confirmed_parameters().is_none());
}

// ---------------------------------------------------------------------------
// Async extraction test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn async_extract_with_no_llm() {
    let registry = load_test_registry();
    let context = SageContext::empty();
    let result = extract_parameters(
        "all conditions must be met before proceeding",
        "conjunctive-gate",
        "1.0.0",
        &context,
        &registry,
        None,
    )
    .await
    .unwrap();

    assert!(
        !result.proposed_parameters.is_empty(),
        "expected at least one proposal"
    );
    assert_eq!(result.pack_name, "conjunctive-gate");
    assert_eq!(result.pack_version, "1.0.0");
    assert!(!result.preview_dsl.is_empty(), "preview DSL should not be empty");
}

#[tokio::test]
async fn async_extract_unknown_pack_returns_error() {
    let registry = load_test_registry();
    let context = SageContext::empty();
    let result = extract_parameters(
        "some utterance",
        "nonexistent-pack",
        "9.9.9",
        &context,
        &registry,
        None,
    )
    .await;
    assert!(result.is_err(), "expected Err for unknown pack");
}
