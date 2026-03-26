//! Integration tests for the unified session pipeline tollgates.
//!
//! Validates that the REPL V2 orchestrator enforces the mandatory gate sequence:
//! ScopeGate → WorkspaceSelection → JourneySelection → InPack
//!
//! These tests mock the agent MCP layer and exercise the orchestrator directly,
//! verifying state transitions and response types at each gate.

use std::sync::Arc;
use uuid::Uuid;

use ob_poc::journey::pack::load_pack_from_bytes;
use ob_poc::journey::router::PackRouter;
use ob_poc::repl::orchestrator_v2::{ReplOrchestratorV2, StubExecutor};
use ob_poc::repl::response_v2::ReplResponseKindV2;
use ob_poc::repl::types_v2::{ReplStateV2, UserInputV2, WorkspaceKind};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_orchestrator() -> ReplOrchestratorV2 {
    let packs = vec![
        {
            let yaml = include_bytes!("../config/packs/onboarding-request.yaml");
            let (m, h) = load_pack_from_bytes(yaml).unwrap();
            (Arc::new(m), h)
        },
        {
            let yaml = include_bytes!("../config/packs/book-setup.yaml");
            let (m, h) = load_pack_from_bytes(yaml).unwrap();
            (Arc::new(m), h)
        },
        {
            let yaml = include_bytes!("../config/packs/kyc-case.yaml");
            let (m, h) = load_pack_from_bytes(yaml).unwrap();
            (Arc::new(m), h)
        },
    ];
    let router = PackRouter::new(packs);
    ReplOrchestratorV2::new(router, Arc::new(StubExecutor))
}

// ---------------------------------------------------------------------------
// Test 1: New session starts in ScopeGate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn new_session_starts_in_scope_gate() {
    let orch = make_orchestrator();
    let id = orch.create_session().await;
    let session = orch.get_session(id).await.unwrap();
    assert!(
        matches!(session.state, ReplStateV2::ScopeGate { .. }),
        "New session must start in ScopeGate, got: {:?}",
        session.state
    );
}

// ---------------------------------------------------------------------------
// Test 2: Utterance in ScopeGate returns ScopeRequired
// ---------------------------------------------------------------------------

#[tokio::test]
async fn utterance_in_scope_gate_returns_scope_required() {
    let orch = make_orchestrator();
    let id = orch.create_session().await;

    let resp = orch
        .process(id, UserInputV2::Message {
            content: "hello".to_string(),
        })
        .await
        .unwrap();

    // Should stay in ScopeGate and prompt for client group
    assert!(
        matches!(resp.kind, ReplResponseKindV2::ScopeRequired { .. }),
        "Expected ScopeRequired, got: {:?}",
        resp.kind
    );
    assert!(
        matches!(resp.state, ReplStateV2::ScopeGate { .. }),
        "Should stay in ScopeGate"
    );
}

// ---------------------------------------------------------------------------
// Test 3: SelectScope transitions to WorkspaceSelection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn select_scope_transitions_to_workspace_selection() {
    let orch = make_orchestrator();
    let id = orch.create_session().await;

    let resp = orch
        .process(id, UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Test Group".to_string(),
        })
        .await
        .unwrap();

    assert!(
        matches!(resp.kind, ReplResponseKindV2::WorkspaceOptions { .. }),
        "Expected WorkspaceOptions after scope selection, got: {:?}",
        resp.kind
    );
    assert!(
        matches!(resp.state, ReplStateV2::WorkspaceSelection { .. }),
        "Should be in WorkspaceSelection"
    );

    // Verify workspace options are present
    if let ReplResponseKindV2::WorkspaceOptions { workspaces } = &resp.kind {
        assert!(!workspaces.is_empty(), "Workspace options must not be empty");
        let labels: Vec<&str> = workspaces.iter().map(|w| w.label.as_str()).collect();
        assert!(labels.contains(&"CBU"), "Must include CBU workspace");
        assert!(labels.contains(&"KYC"), "Must include KYC workspace");
        assert!(labels.contains(&"Deal"), "Must include Deal workspace");
    }
}

// ---------------------------------------------------------------------------
// Test 4: SelectWorkspace transitions to JourneySelection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn select_workspace_transitions_to_journey_selection() {
    let orch = make_orchestrator();
    let id = orch.create_session().await;

    // Pass scope gate
    orch.process(id, UserInputV2::SelectScope {
        group_id: Uuid::new_v4(),
        group_name: "Test Group".to_string(),
    })
    .await
    .unwrap();

    // Select workspace
    let resp = orch
        .process(id, UserInputV2::SelectWorkspace {
            workspace: WorkspaceKind::Cbu,
        })
        .await
        .unwrap();

    assert!(
        matches!(resp.kind, ReplResponseKindV2::JourneyOptions { .. }),
        "Expected JourneyOptions after workspace selection, got: {:?}",
        resp.kind
    );
    assert!(
        matches!(resp.state, ReplStateV2::JourneySelection { .. }),
        "Should be in JourneySelection"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Cannot skip scope gate — workspace selection rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cannot_skip_scope_gate() {
    let orch = make_orchestrator();
    let id = orch.create_session().await;

    // Try to select workspace directly (skipping scope)
    let resp = orch
        .process(id, UserInputV2::SelectWorkspace {
            workspace: WorkspaceKind::Cbu,
        })
        .await
        .unwrap();

    // Should stay in ScopeGate
    assert!(
        matches!(resp.state, ReplStateV2::ScopeGate { .. }),
        "Must stay in ScopeGate when workspace selected without scope, got: {:?}",
        resp.state
    );
}

// ---------------------------------------------------------------------------
// Test 6: Full tollgate flow — scope → workspace → journey → InPack
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_tollgate_flow_to_in_pack() {
    let orch = make_orchestrator();
    let id = orch.create_session().await;

    // Gate 1: ScopeGate → WorkspaceSelection
    let resp = orch
        .process(id, UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Allianz".to_string(),
        })
        .await
        .unwrap();
    assert!(matches!(resp.state, ReplStateV2::WorkspaceSelection { .. }));

    // Gate 2: WorkspaceSelection → JourneySelection
    let resp = orch
        .process(id, UserInputV2::SelectWorkspace {
            workspace: WorkspaceKind::OnBoarding,
        })
        .await
        .unwrap();
    assert!(matches!(resp.state, ReplStateV2::JourneySelection { .. }));

    // Gate 3: JourneySelection → InPack (select a pack matching the workspace)
    let resp = orch
        .process(id, UserInputV2::SelectPack {
            pack_id: "onboarding-request".to_string(),
        })
        .await
        .unwrap();

    // Should now be in InPack or asking the first question
    let in_pack_or_question = matches!(resp.state, ReplStateV2::InPack { .. })
        || matches!(resp.kind, ReplResponseKindV2::Question { .. })
        || matches!(resp.kind, ReplResponseKindV2::Prompt { .. });
    assert!(
        in_pack_or_question,
        "After pack selection should be InPack or asking first question, got state: {:?}, kind: {:?}",
        resp.state, resp.kind
    );
}

// ---------------------------------------------------------------------------
// Test 7: Session feedback populated on every response
// ---------------------------------------------------------------------------

#[tokio::test]
async fn session_feedback_populated_after_scope() {
    let orch = make_orchestrator();
    let id = orch.create_session().await;

    // Pass scope gate
    let resp = orch
        .process(id, UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Allianz".to_string(),
        })
        .await
        .unwrap();

    assert!(
        resp.session_feedback.is_some(),
        "SessionFeedback must be populated after scope gate"
    );
}

// ---------------------------------------------------------------------------
// Test 8: Trace entries recorded through tollgates
// ---------------------------------------------------------------------------

#[tokio::test]
async fn trace_entries_recorded_through_gates() {
    let orch = make_orchestrator();
    let id = orch.create_session().await;

    // Send a message (generates Input trace)
    orch.process(id, UserInputV2::Message {
        content: "hello".to_string(),
    })
    .await
    .unwrap();

    // Pass scope gate (generates StateTransition trace)
    orch.process(id, UserInputV2::SelectScope {
        group_id: Uuid::new_v4(),
        group_name: "Test".to_string(),
    })
    .await
    .unwrap();

    let session = orch.get_session(id).await.unwrap();
    assert!(
        session.trace_sequence >= 2,
        "Expected at least 2 trace entries, got {}",
        session.trace_sequence
    );
}

// ---------------------------------------------------------------------------
// Test 9: Response adapter produces valid ChatResponse
// ---------------------------------------------------------------------------

#[tokio::test]
async fn response_adapter_produces_valid_chat_response() {
    use ob_poc::api::response_adapter::repl_to_chat_response;

    let orch = make_orchestrator();
    let id = orch.create_session().await;

    // ScopeGate response
    let resp = orch
        .process(id, UserInputV2::Message {
            content: "hi".to_string(),
        })
        .await
        .unwrap();

    let chat = repl_to_chat_response(resp, id);
    assert!(chat.decision.is_some(), "ScopeGate should produce a decision packet");
    assert!(!chat.message.is_empty(), "Message must not be empty");

    // WorkspaceSelection response
    orch.process(id, UserInputV2::SelectScope {
        group_id: Uuid::new_v4(),
        group_name: "Test".to_string(),
    })
    .await
    .unwrap();

    let session = orch.get_session(id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::WorkspaceSelection { .. }));
}
