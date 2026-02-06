//! Phase 5 Integration Tests — Durable Execution + Human Gates
//!
//! These tests verify Phase 5 functionality:
//!
//! Runbook Model Tests (1-6):
//!  1. park_entry sets Parked status and emits EntryParked event
//!  2. resume_entry sets Completed and emits EntryResumed event
//!  3. resume_entry with unknown key returns None
//!  4. park then resume is idempotent (second resume returns None)
//!  5. InvocationRecord serialization roundtrip
//!  6. rebuild_invocation_index restores from entries
//!
//! DslExecutorV2 Tests (7-9):
//!  7. StubExecutor adapts via blanket impl on DslExecutorV2
//!  8. ParkableStubExecutor parks on :park marker
//!  9. ParkableStubExecutor completes on normal DSL
//!
//! Execute Mode Routing (10-15):
//! 10. Sync entries execute unchanged (regression)
//! 11. Durable entry parks on Parked outcome
//! 12. HumanGate parks before execution (DSL not called)
//! 13. Mixed mode stops at gate (first sync completes, second parks)
//! 14. Entries before park complete normally
//! 15. Entries after park remain Confirmed
//!
//! handle_executing() Tests (16-21):
//! 16. Rejects random input when parked
//! 17. Status check returns parked info
//! 18. Approve human gate executes and continues
//! 19. Reject human gate marks failed
//! 20. Cancel aborts all parked
//! 21. Approve then continue executes remaining entries
//!
//! Signal Routing (22-24):
//! 22. Durable park then signal resumes
//! 23. Signal continues remaining entries
//! 24. Duplicate signal is noop
//!
//! Golden Loops (25-28):
//! 25. Golden loop all sync regression
//! 26. Golden loop with durable
//! 27. Golden loop with human gate
//! 28. Golden loop mixed modes
#![cfg(feature = "vnext-repl")]

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use dsl_core::config::types::VerbsConfig;
use ob_poc::journey::pack::{load_pack_from_bytes, PackManifest};
use ob_poc::journey::router::PackRouter;
use ob_poc::repl::intent_matcher::IntentMatcher;
use ob_poc::repl::intent_service::IntentService;
use ob_poc::repl::orchestrator_v2::{
    DslExecutionOutcome, DslExecutorV2, ParkableStubExecutor, ReplOrchestratorV2, StubExecutor,
};
use ob_poc::repl::proposal_engine::ProposalEngine;
use ob_poc::repl::response_v2::ReplResponseKindV2;
use ob_poc::repl::runbook::{
    EntryStatus, ExecutionMode, GateType, InvocationRecord, InvocationStatus, Runbook,
    RunbookEntry, RunbookEvent,
};
use ob_poc::repl::types::{IntentMatchResult, MatchContext, MatchOutcome};
use ob_poc::repl::types_v2::{ReplCommandV2, ReplStateV2, UserInputV2};
use ob_poc::repl::verb_config_index::VerbConfigIndex;

// ===========================================================================
// Helpers (shared with Phase 3/4 test patterns)
// ===========================================================================

fn load_cbu_verbs_config() -> VerbsConfig {
    let yaml = include_str!("../config/verbs/cbu.yaml");
    serde_yaml::from_str(yaml).expect("cbu.yaml should parse as VerbsConfig")
}

fn load_session_verbs_config() -> VerbsConfig {
    let yaml = include_str!("../config/verbs/session.yaml");
    serde_yaml::from_str(yaml).expect("session.yaml should parse as VerbsConfig")
}

fn merge_configs(configs: Vec<VerbsConfig>) -> VerbsConfig {
    let mut merged = VerbsConfig {
        version: "1".to_string(),
        domains: HashMap::new(),
    };
    for config in configs {
        for (domain_name, domain) in config.domains {
            merged
                .domains
                .entry(domain_name)
                .and_modify(|existing| {
                    existing.verbs.extend(domain.verbs.clone());
                })
                .or_insert(domain);
        }
    }
    merged
}

fn build_real_index() -> VerbConfigIndex {
    let config = merge_configs(vec![load_cbu_verbs_config(), load_session_verbs_config()]);
    VerbConfigIndex::from_verbs_config(&config)
}

fn build_freeform_pack() -> (Arc<PackManifest>, String) {
    let yaml = r#"
id: freeform-test
name: Freeform Test Pack
version: "1.0"
description: "Test pack with no templates or required questions"
invocation_phrases:
  - "freeform test"
required_context: []
optional_context: []
allowed_verbs: []
forbidden_verbs: []
risk_policy:
  max_risk_score: 5
required_questions: []
optional_questions: []
stop_rules: []
templates: []
section_layout: []
definition_of_done: []
progress_signals: []
"#;
    let (manifest, hash) = load_pack_from_bytes(yaml.as_bytes()).unwrap();
    (Arc::new(manifest), hash)
}

/// A mock IntentMatcher that returns configurable results.
#[derive(Clone)]
struct MockIntentMatcher {
    result: IntentMatchResult,
}

impl MockIntentMatcher {
    fn matched(verb: &str, confidence: f32, dsl: Option<&str>) -> Self {
        Self {
            result: IntentMatchResult {
                outcome: MatchOutcome::Matched {
                    verb: verb.to_string(),
                    confidence,
                },
                verb_candidates: vec![],
                entity_mentions: vec![],
                scope_candidates: None,
                generated_dsl: dsl.map(|s| s.to_string()),
                unresolved_refs: vec![],
                debug: None,
            },
        }
    }

    #[allow(dead_code)]
    fn ambiguous(candidates: Vec<(&str, f32)>, margin: f32) -> Self {
        Self {
            result: IntentMatchResult {
                outcome: MatchOutcome::Ambiguous { margin },
                verb_candidates: candidates
                    .into_iter()
                    .map(|(v, s)| ob_poc::repl::types::VerbCandidate {
                        verb_fqn: v.to_string(),
                        description: format!("Description for {}", v),
                        score: s,
                        example: None,
                        domain: Some(v.split('.').next().unwrap_or("").to_string()),
                    })
                    .collect(),
                entity_mentions: vec![],
                scope_candidates: None,
                generated_dsl: None,
                unresolved_refs: vec![],
                debug: None,
            },
        }
    }

    #[allow(dead_code)]
    fn no_match() -> Self {
        Self {
            result: IntentMatchResult {
                outcome: MatchOutcome::NoMatch {
                    reason: "No matching verb".to_string(),
                },
                verb_candidates: vec![],
                entity_mentions: vec![],
                scope_candidates: None,
                generated_dsl: None,
                unresolved_refs: vec![],
                debug: None,
            },
        }
    }
}

#[async_trait]
impl IntentMatcher for MockIntentMatcher {
    async fn match_intent(
        &self,
        _input: &str,
        _ctx: &MatchContext,
    ) -> anyhow::Result<IntentMatchResult> {
        Ok(self.result.clone())
    }
}

/// Build an orchestrator with proposal engine and StubExecutor (sync-only).
fn build_orchestrator_with_engine(matcher: MockIntentMatcher) -> ReplOrchestratorV2 {
    let index = Arc::new(build_real_index());
    let intent_matcher: Arc<dyn IntentMatcher> = Arc::new(matcher.clone());
    let intent_service = Arc::new(IntentService::new(intent_matcher.clone(), index.clone()));
    let engine = Arc::new(ProposalEngine::new(intent_service.clone(), index.clone()));

    let (pack, hash) = build_freeform_pack();
    let router = PackRouter::new(vec![(pack, hash)]);

    ReplOrchestratorV2::new(router, Arc::new(StubExecutor))
        .with_verb_config_index(index)
        .with_intent_matcher(intent_matcher)
        .with_intent_service(intent_service)
        .with_proposal_engine(engine)
}

/// Build an orchestrator with proposal engine and ParkableStubExecutor (extended).
fn build_orchestrator_with_parkable_executor(matcher: MockIntentMatcher) -> ReplOrchestratorV2 {
    let index = Arc::new(build_real_index());
    let intent_matcher: Arc<dyn IntentMatcher> = Arc::new(matcher.clone());
    let intent_service = Arc::new(IntentService::new(intent_matcher.clone(), index.clone()));
    let engine = Arc::new(ProposalEngine::new(intent_service.clone(), index.clone()));

    let (pack, hash) = build_freeform_pack();
    let router = PackRouter::new(vec![(pack, hash)]);

    ReplOrchestratorV2::new(router, Arc::new(StubExecutor))
        .with_verb_config_index(index)
        .with_intent_matcher(intent_matcher)
        .with_intent_service(intent_service)
        .with_proposal_engine(engine)
        .with_executor_v2(Arc::new(ParkableStubExecutor))
}

/// Helper: scope + pack selection -> session lands in InPack.
async fn setup_in_pack(orch: &ReplOrchestratorV2) -> Uuid {
    let session_id = orch.create_session().await;

    // Set scope
    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Allianz".to_string(),
        },
    )
    .await
    .unwrap();

    // Select freeform pack
    orch.process(
        session_id,
        UserInputV2::SelectPack {
            pack_id: "freeform-test".to_string(),
        },
    )
    .await
    .unwrap();

    // Verify we're in InPack
    let session = orch.get_session(session_id).await.unwrap();
    assert!(
        matches!(session.state, ReplStateV2::InPack { .. }),
        "After setup, session should be in InPack, got: {:?}",
        std::mem::discriminant(&session.state)
    );

    session_id
}

/// Helper: set up InPack session and add a confirmed entry to the runbook.
async fn setup_with_one_entry(orch: &ReplOrchestratorV2) -> (Uuid, Uuid) {
    let session_id = setup_in_pack(orch).await;

    // Send message to get a proposal
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "create allianz lux cbu".to_string(),
            },
        )
        .await
        .unwrap();

    // Should auto-advance to SentencePlayback (high confidence mock)
    assert!(
        matches!(resp.kind, ReplResponseKindV2::SentencePlayback { .. }),
        "Expected SentencePlayback, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );

    // Confirm the sentence
    let resp = orch
        .process(session_id, UserInputV2::Confirm)
        .await
        .unwrap();

    assert!(
        matches!(resp.kind, ReplResponseKindV2::RunbookSummary { .. }),
        "Expected RunbookSummary after confirm, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );

    // Get the entry ID
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries.len(), 1);
    let entry_id = session.runbook.entries[0].id;

    (session_id, entry_id)
}

/// Helper: add multiple confirmed entries to a session.
/// Returns the session_id and a vec of entry_ids.
async fn setup_with_n_entries(orch: &ReplOrchestratorV2, n: usize) -> (Uuid, Vec<Uuid>) {
    let session_id = setup_in_pack(orch).await;
    let mut entry_ids = Vec::new();

    for i in 0..n {
        let resp = orch
            .process(
                session_id,
                UserInputV2::Message {
                    content: format!("create cbu number {}", i + 1),
                },
            )
            .await
            .unwrap();

        assert!(
            matches!(resp.kind, ReplResponseKindV2::SentencePlayback { .. }),
            "Expected SentencePlayback for entry {}, got: {:?}",
            i,
            std::mem::discriminant(&resp.kind)
        );

        orch.process(session_id, UserInputV2::Confirm)
            .await
            .unwrap();
    }

    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries.len(), n);
    for entry in &session.runbook.entries {
        entry_ids.push(entry.id);
    }

    (session_id, entry_ids)
}

/// Create a sample RunbookEntry for unit tests.
fn sample_entry(verb: &str, sentence: &str) -> RunbookEntry {
    RunbookEntry::new(
        verb.to_string(),
        sentence.to_string(),
        format!("({} :placeholder true)", verb),
    )
}

// ===========================================================================
// TEST 1: park_entry sets Parked status and emits EntryParked event
// ===========================================================================

#[test]
fn test_park_entry_sets_parked_status_and_emits_event() {
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("doc.solicit", "Request passport");
    entry.status = EntryStatus::Confirmed;
    let entry_id = rb.add_entry(entry);

    let inv = InvocationRecord::new(
        entry_id,
        rb.id,
        rb.session_id,
        InvocationRecord::make_correlation_key(rb.id, entry_id),
        GateType::DurableTask,
    );

    assert!(rb.park_entry(entry_id, inv));

    let entry = rb.entry_by_id(entry_id).unwrap();
    assert_eq!(entry.status, EntryStatus::Parked);
    assert!(entry.invocation.is_some());

    // Check invocation_index has the correlation key
    let corr_key = InvocationRecord::make_correlation_key(rb.id, entry_id);
    assert_eq!(rb.invocation_index.get(&corr_key), Some(&entry_id));

    // Check audit trail has EntryParked event
    let parked_events: Vec<_> = rb
        .audit
        .iter()
        .filter(|e| matches!(e, RunbookEvent::EntryParked { .. }))
        .collect();
    assert_eq!(parked_events.len(), 1);
}

// ===========================================================================
// TEST 2: resume_entry sets Completed and emits EntryResumed event
// ===========================================================================

#[test]
fn test_resume_entry_sets_completed_and_emits_event() {
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("doc.solicit", "Request passport");
    entry.status = EntryStatus::Confirmed;
    let entry_id = rb.add_entry(entry);

    let corr_key = InvocationRecord::make_correlation_key(rb.id, entry_id);
    let inv = InvocationRecord::new(
        entry_id,
        rb.id,
        rb.session_id,
        corr_key.clone(),
        GateType::DurableTask,
    );
    rb.park_entry(entry_id, inv);

    let result = serde_json::json!({"doc_id": "abc-123"});
    let resumed_id = rb.resume_entry(&corr_key, Some(result.clone()));
    assert_eq!(resumed_id, Some(entry_id));

    let entry = rb.entry_by_id(entry_id).unwrap();
    assert_eq!(entry.status, EntryStatus::Completed);
    assert_eq!(entry.result, Some(result));
    assert_eq!(
        entry.invocation.as_ref().unwrap().status,
        InvocationStatus::Completed
    );
    assert!(entry.invocation.as_ref().unwrap().resumed_at.is_some());

    // Correlation key removed from index
    assert!(rb.invocation_index.is_empty());

    // Check audit trail has EntryResumed event
    let resumed_events: Vec<_> = rb
        .audit
        .iter()
        .filter(|e| matches!(e, RunbookEvent::EntryResumed { .. }))
        .collect();
    assert_eq!(resumed_events.len(), 1);
}

// ===========================================================================
// TEST 3: resume_entry with unknown key returns None
// ===========================================================================

#[test]
fn test_resume_unknown_key_returns_none() {
    let mut rb = Runbook::new(Uuid::new_v4());
    assert!(rb.resume_entry("nonexistent:key", None).is_none());
}

// ===========================================================================
// TEST 4: park then resume is idempotent (second resume returns None)
// ===========================================================================

#[test]
fn test_park_then_resume_is_idempotent() {
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("doc.solicit", "Request passport");
    entry.status = EntryStatus::Confirmed;
    let entry_id = rb.add_entry(entry);

    let corr_key = InvocationRecord::make_correlation_key(rb.id, entry_id);
    let inv = InvocationRecord::new(
        entry_id,
        rb.id,
        rb.session_id,
        corr_key.clone(),
        GateType::DurableTask,
    );
    rb.park_entry(entry_id, inv);

    // First resume succeeds
    assert!(rb.resume_entry(&corr_key, None).is_some());
    // Second resume is no-op (correlation key already removed)
    assert!(rb.resume_entry(&corr_key, None).is_none());
}

// ===========================================================================
// TEST 5: InvocationRecord serialization roundtrip
// ===========================================================================

#[test]
fn test_invocation_record_serialization_roundtrip() {
    let inv = InvocationRecord::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        "test:correlation:key".to_string(),
        GateType::HumanApproval,
    );

    let json = serde_json::to_string(&inv).unwrap();
    let deserialized: InvocationRecord = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.invocation_id, inv.invocation_id);
    assert_eq!(deserialized.entry_id, inv.entry_id);
    assert_eq!(deserialized.runbook_id, inv.runbook_id);
    assert_eq!(deserialized.correlation_key, inv.correlation_key);
    assert_eq!(deserialized.gate_type, GateType::HumanApproval);
    assert_eq!(deserialized.status, InvocationStatus::Active);
}

// ===========================================================================
// TEST 6: rebuild_invocation_index restores from entries
// ===========================================================================

#[test]
fn test_rebuild_invocation_index_restores_from_entries() {
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("doc.solicit", "Request passport");
    entry.status = EntryStatus::Confirmed;
    let entry_id = rb.add_entry(entry);

    let corr_key = InvocationRecord::make_correlation_key(rb.id, entry_id);
    let inv = InvocationRecord::new(
        entry_id,
        rb.id,
        rb.session_id,
        corr_key.clone(),
        GateType::DurableTask,
    );
    rb.park_entry(entry_id, inv);

    // Simulate deserialization: clear the index
    rb.invocation_index.clear();
    assert!(rb.invocation_index.is_empty());

    // Rebuild should restore it
    rb.rebuild_invocation_index();
    assert_eq!(rb.invocation_index.get(&corr_key), Some(&entry_id));
}

// ===========================================================================
// TEST 7: StubExecutor adapts via blanket impl on DslExecutorV2
// ===========================================================================

#[tokio::test]
async fn test_stub_executor_adapts_via_blanket_impl() {
    let exec = StubExecutor;
    let outcome = exec
        .execute_v2(
            "(cbu.create :name \"test\")",
            Uuid::new_v4(),
            Uuid::new_v4(),
        )
        .await;
    match outcome {
        DslExecutionOutcome::Completed(v) => {
            assert_eq!(v["status"], "stub_success");
        }
        other => panic!("Expected Completed, got {:?}", other),
    }
}

// ===========================================================================
// TEST 8: ParkableStubExecutor parks on :park marker
// ===========================================================================

#[tokio::test]
async fn test_parkable_executor_parks_on_marker() {
    let exec = ParkableStubExecutor;
    let entry_id = Uuid::new_v4();
    let runbook_id = Uuid::new_v4();
    let outcome = exec
        .execute_v2("(cbu.create :park)", entry_id, runbook_id)
        .await;
    match outcome {
        DslExecutionOutcome::Parked {
            correlation_key,
            message,
            ..
        } => {
            assert!(correlation_key.contains(&runbook_id.to_string()));
            assert!(correlation_key.contains(&entry_id.to_string()));
            assert!(!message.is_empty());
        }
        other => panic!("Expected Parked, got {:?}", other),
    }
}

// ===========================================================================
// TEST 9: ParkableStubExecutor completes on normal DSL
// ===========================================================================

#[tokio::test]
async fn test_parkable_executor_completes_on_normal_dsl() {
    let exec = ParkableStubExecutor;
    let outcome = exec
        .execute_v2(
            "(cbu.create :name \"test\")",
            Uuid::new_v4(),
            Uuid::new_v4(),
        )
        .await;
    match outcome {
        DslExecutionOutcome::Completed(v) => {
            assert_eq!(v["status"], "stub_success");
        }
        other => panic!("Expected Completed, got {:?}", other),
    }
}

// ===========================================================================
// TEST 10: Sync entries execute unchanged (regression)
// ===========================================================================

#[tokio::test]
async fn test_sync_entries_execute_unchanged() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, _entry_id) = setup_with_one_entry(&orch).await;

    // All entries default to Sync mode. Run should execute normally.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            },
        )
        .await
        .unwrap();

    match &resp.kind {
        ReplResponseKindV2::Executed { results } => {
            assert_eq!(results.len(), 1);
            assert!(results[0].success);
            assert_eq!(results[0].message.as_deref(), Some("Completed"));
        }
        other => panic!(
            "Expected Executed response, got: {:?}",
            std::mem::discriminant(other)
        ),
    }

    // Session should be back in RunbookEditing
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}

// ===========================================================================
// TEST 11: Durable entry parks on Parked outcome
// ===========================================================================

#[tokio::test]
async fn test_durable_entry_parks_on_parked_outcome() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Set entry to Durable mode with :park marker in DSL
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::Durable;
        session.runbook.entries[0].dsl = "(doc.solicit :park :entity-id \"test\")".to_string();
    }

    // Run
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            },
        )
        .await
        .unwrap();

    // Should report execution with parked entry
    assert!(
        resp.message.contains("parked") || resp.message.contains("Parked"),
        "Expected parked message, got: {}",
        resp.message
    );

    // Session should be in Executing state with parked_steps=1
    let session = orch.get_session(session_id).await.unwrap();
    match &session.state {
        ReplStateV2::Executing { progress, .. } => {
            assert_eq!(progress.parked_steps, 1);
            assert_eq!(progress.parked_entry_id, Some(entry_id));
        }
        other => panic!(
            "Expected Executing state, got: {:?}",
            std::mem::discriminant(other)
        ),
    }

    // Entry should be Parked
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Parked
    );
}

// ===========================================================================
// TEST 12: HumanGate parks before execution (DSL not called)
// ===========================================================================

#[tokio::test]
async fn test_human_gate_parks_before_execution() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Set entry to HumanGate mode
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::HumanGate;
    }

    // Run
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            },
        )
        .await
        .unwrap();

    // Should report parked with human approval message
    assert!(
        resp.message.contains("parked")
            || resp.message.contains("Parked")
            || resp.message.contains("approval"),
        "Expected parked/approval message, got: {}",
        resp.message
    );

    // Session should be in Executing state
    let session = orch.get_session(session_id).await.unwrap();
    assert!(
        matches!(session.state, ReplStateV2::Executing { .. }),
        "Expected Executing state, got: {:?}",
        std::mem::discriminant(&session.state)
    );

    // Entry should be Parked with HumanApproval gate type
    let entry = session.runbook.entry_by_id(entry_id).unwrap();
    assert_eq!(entry.status, EntryStatus::Parked);
    assert!(entry.invocation.is_some());
    assert_eq!(
        entry.invocation.as_ref().unwrap().gate_type,
        GateType::HumanApproval
    );
}

// ===========================================================================
// TEST 13: Mixed mode stops at gate (first sync completes, second parks)
// ===========================================================================

#[tokio::test]
async fn test_mixed_mode_stops_at_gate() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, entry_ids) = setup_with_n_entries(&orch, 2).await;

    // Entry 1: Sync (default), Entry 2: HumanGate
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[1].execution_mode = ExecutionMode::HumanGate;
    }

    // Run
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            },
        )
        .await
        .unwrap();

    // Should have results for both entries
    match &resp.kind {
        ReplResponseKindV2::Executed { results } => {
            assert_eq!(results.len(), 2);

            // First entry should be completed
            let r1 = results.iter().find(|r| r.entry_id == entry_ids[0]).unwrap();
            assert!(r1.success);
            assert_eq!(r1.message.as_deref(), Some("Completed"));

            // Second entry should be parked (awaiting approval)
            let r2 = results.iter().find(|r| r.entry_id == entry_ids[1]).unwrap();
            assert!(r2.success); // Parked entries are reported as success in results
            assert!(
                r2.message
                    .as_ref()
                    .map(|m| m.contains("approval"))
                    .unwrap_or(false),
                "Expected approval message, got: {:?}",
                r2.message
            );
        }
        other => panic!(
            "Expected Executed response, got: {:?}",
            std::mem::discriminant(other)
        ),
    }

    // Session should be in Executing (parked)
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::Executing { .. }));

    // Entry 1 completed, Entry 2 parked
    assert_eq!(session.runbook.entries[0].status, EntryStatus::Completed);
    assert_eq!(session.runbook.entries[1].status, EntryStatus::Parked);
}

// ===========================================================================
// TEST 14: Entries before park complete normally
// ===========================================================================

#[tokio::test]
async fn test_entries_before_park_complete_normally() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, _entry_ids) = setup_with_n_entries(&orch, 2).await;

    // Entry 1: Sync, Entry 2: Durable with :park marker
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[1].execution_mode = ExecutionMode::Durable;
        session.runbook.entries[1].dsl = "(doc.solicit :park)".to_string();
    }

    // Run
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    let session = orch.get_session(session_id).await.unwrap();

    // Entry 1 completed normally
    assert_eq!(session.runbook.entries[0].status, EntryStatus::Completed);
    assert!(session.runbook.entries[0].result.is_some());

    // Entry 2 parked
    assert_eq!(session.runbook.entries[1].status, EntryStatus::Parked);
}

// ===========================================================================
// TEST 15: Entries after park remain Confirmed
// ===========================================================================

#[tokio::test]
async fn test_entries_after_park_remain_confirmed() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, _entry_ids) = setup_with_n_entries(&orch, 2).await;

    // Entry 1: Durable with :park, Entry 2: Sync
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::Durable;
        session.runbook.entries[0].dsl = "(doc.solicit :park)".to_string();
        // Entry 2 stays Sync (default)
    }

    // Run
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    let session = orch.get_session(session_id).await.unwrap();

    // Entry 1 parked
    assert_eq!(session.runbook.entries[0].status, EntryStatus::Parked);

    // Entry 2 still Confirmed (not executed yet because we stopped at park)
    assert_eq!(session.runbook.entries[1].status, EntryStatus::Confirmed);
}

// ===========================================================================
// TEST 16: Rejects random input when parked
// ===========================================================================

#[tokio::test]
async fn test_rejects_random_input_when_parked() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, _entry_id) = setup_with_one_entry(&orch).await;

    // Set to HumanGate and run to park
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::HumanGate;
    }

    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // Verify we're in Executing (parked)
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::Executing { .. }));

    // Send a random message
    let resp = orch
        .process(session_id, UserInputV2::Confirm)
        .await
        .unwrap();

    assert!(
        matches!(
            resp.kind,
            ReplResponseKindV2::Error {
                recoverable: true,
                ..
            }
        ),
        "Random input when parked should return error, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );
    assert!(
        resp.message.contains("parked") || resp.message.contains("Parked"),
        "Error message should mention parked state, got: {}",
        resp.message
    );
}

// ===========================================================================
// TEST 17: Status check returns parked info
// ===========================================================================

#[tokio::test]
async fn test_status_check_returns_parked_info() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, _entry_id) = setup_with_one_entry(&orch).await;

    // Set to HumanGate and run to park
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::HumanGate;
    }

    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // Send Status command
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Status,
            },
        )
        .await
        .unwrap();

    // Should return RunbookSummary with parked entry info
    assert!(
        matches!(resp.kind, ReplResponseKindV2::RunbookSummary { .. }),
        "Expected RunbookSummary for status, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );
    assert!(
        resp.message.contains("Parked") || resp.message.contains("parked"),
        "Status message should mention parked entries, got: {}",
        resp.message
    );
    assert!(
        resp.message.contains("HumanApproval") || resp.message.contains("gate"),
        "Status message should mention gate type, got: {}",
        resp.message
    );
}

// ===========================================================================
// TEST 18: Approve human gate executes and continues
// ===========================================================================

#[tokio::test]
async fn test_approve_human_gate_executes_and_continues() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Set to HumanGate and run to park
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::HumanGate;
    }

    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // Verify entry is parked
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Parked
    );

    // Approve the gate
    orch.process(
        session_id,
        UserInputV2::Approve {
            entry_id,
            approved_by: Some("tester".to_string()),
        },
    )
    .await
    .unwrap();

    // After approval, entry should be executed and session back to RunbookEditing
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Completed
    );
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}

// ===========================================================================
// TEST 19: Reject human gate marks failed
// ===========================================================================

#[tokio::test]
async fn test_reject_human_gate_marks_failed() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Set to HumanGate and run to park
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::HumanGate;
    }

    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // Reject the gate
    let resp = orch
        .process(
            session_id,
            UserInputV2::RejectGate {
                entry_id,
                reason: Some("Not ready yet".to_string()),
            },
        )
        .await
        .unwrap();

    assert!(
        resp.message.contains("rejected") || resp.message.contains("Rejected"),
        "Expected rejection message, got: {}",
        resp.message
    );

    // Entry should be Failed
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Failed
    );

    // Session should be back to RunbookEditing
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));

    // Check rejection reason in result
    let result = session
        .runbook
        .entry_by_id(entry_id)
        .unwrap()
        .result
        .as_ref()
        .unwrap();
    assert_eq!(result["rejected"], true);
    assert_eq!(result["reason"], "Not ready yet");
}

// ===========================================================================
// TEST 20: Cancel aborts all parked
// ===========================================================================

#[tokio::test]
async fn test_cancel_aborts_all_parked() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Set to HumanGate and run to park
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::HumanGate;
    }

    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // Cancel
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Cancel,
            },
        )
        .await
        .unwrap();

    assert!(
        resp.message.contains("cancelled") || resp.message.contains("Cancelled"),
        "Expected cancellation message, got: {}",
        resp.message
    );

    // Entry should be Failed (cancelled)
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Failed
    );

    // Session should be back to RunbookEditing
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));

    // Invocation index should be empty
    assert!(session.runbook.invocation_index.is_empty());
}

// ===========================================================================
// TEST 21: Approve then continue executes remaining entries
// ===========================================================================

#[tokio::test]
async fn test_approve_then_continue_executes_remaining() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, entry_ids) = setup_with_n_entries(&orch, 3).await;

    // Entry 1: Sync, Entry 2: HumanGate, Entry 3: Sync
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[1].execution_mode = ExecutionMode::HumanGate;
    }

    // Run -> Entry 1 completes, Entry 2 parks
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // Verify: Entry 1 completed, Entry 2 parked, Entry 3 still Confirmed
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries[0].status, EntryStatus::Completed);
    assert_eq!(session.runbook.entries[1].status, EntryStatus::Parked);
    assert_eq!(session.runbook.entries[2].status, EntryStatus::Confirmed);

    // Approve Entry 2
    orch.process(
        session_id,
        UserInputV2::Approve {
            entry_id: entry_ids[1],
            approved_by: Some("tester".to_string()),
        },
    )
    .await
    .unwrap();

    // After approval, all entries should be completed
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries[0].status, EntryStatus::Completed);
    assert_eq!(session.runbook.entries[1].status, EntryStatus::Completed);
    assert_eq!(session.runbook.entries[2].status, EntryStatus::Completed);

    // Session should be back to RunbookEditing
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}

// ===========================================================================
// TEST 22: Durable park then signal resumes
// ===========================================================================

#[tokio::test]
async fn test_durable_park_then_signal_resumes() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Set to Durable with :park marker
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::Durable;
        session.runbook.entries[0].dsl = "(doc.solicit :park)".to_string();
    }

    // Run -> parks
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // Verify parked
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Parked
    );

    // Find the correlation key
    let correlation_key = {
        let session = orch.get_session(session_id).await.unwrap();
        session
            .runbook
            .entry_by_id(entry_id)
            .unwrap()
            .invocation
            .as_ref()
            .unwrap()
            .correlation_key
            .clone()
    };

    // Resume the entry by directly calling resume_entry on the runbook
    // (simulates an inbound signal from external system)
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        let result = serde_json::json!({"doc_id": "received-123"});
        let resumed = session.runbook.resume_entry(&correlation_key, Some(result));
        assert_eq!(resumed, Some(entry_id));
    }

    // Now continue execution via Resume command
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Resume(entry_id),
        },
    )
    .await
    .unwrap();

    // Session should be back to RunbookEditing
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Completed
    );
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}

// ===========================================================================
// TEST 23: Signal continues remaining entries
// ===========================================================================

#[tokio::test]
async fn test_signal_continues_remaining_entries() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);
    let (session_id, entry_ids) = setup_with_n_entries(&orch, 2).await;

    // Entry 1: Durable with :park, Entry 2: Sync
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::Durable;
        session.runbook.entries[0].dsl = "(doc.solicit :park)".to_string();
        // Entry 2 stays Sync
    }

    // Run -> Entry 1 parks, Entry 2 stays Confirmed
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // Verify
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries[0].status, EntryStatus::Parked);
    assert_eq!(session.runbook.entries[1].status, EntryStatus::Confirmed);

    // Resume Entry 1
    let correlation_key = {
        let session = orch.get_session(session_id).await.unwrap();
        session.runbook.entries[0]
            .invocation
            .as_ref()
            .unwrap()
            .correlation_key
            .clone()
    };

    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session
            .runbook
            .resume_entry(&correlation_key, Some(serde_json::json!({"ok": true})));
    }

    // Continue from Entry 1 -> Entry 2 should execute
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Resume(entry_ids[0]),
        },
    )
    .await
    .unwrap();

    // Both entries should be completed
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries[0].status, EntryStatus::Completed);
    assert_eq!(session.runbook.entries[1].status, EntryStatus::Completed);
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}

// ===========================================================================
// TEST 24: Duplicate signal is noop
// ===========================================================================

#[tokio::test]
async fn test_duplicate_signal_is_noop() {
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("doc.solicit", "Request passport");
    entry.status = EntryStatus::Confirmed;
    let entry_id = rb.add_entry(entry);

    let corr_key = InvocationRecord::make_correlation_key(rb.id, entry_id);
    let inv = InvocationRecord::new(
        entry_id,
        rb.id,
        rb.session_id,
        corr_key.clone(),
        GateType::DurableTask,
    );
    rb.park_entry(entry_id, inv);

    // First resume succeeds
    let result1 = rb.resume_entry(&corr_key, Some(serde_json::json!({"ok": true})));
    assert_eq!(result1, Some(entry_id));

    // Second resume returns None (idempotent)
    let result2 = rb.resume_entry(&corr_key, Some(serde_json::json!({"ok": true})));
    assert_eq!(result2, None);

    // Entry still completed from first resume
    assert_eq!(
        rb.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Completed
    );
}

// ===========================================================================
// TEST 25: Golden loop all sync regression
// ===========================================================================

#[tokio::test]
async fn test_golden_loop_all_sync_regression() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);

    // 1. Create session
    let session_id = orch.create_session().await;

    // 2. Set scope
    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Allianz".to_string(),
        },
    )
    .await
    .unwrap();

    // 3. Select pack
    orch.process(
        session_id,
        UserInputV2::SelectPack {
            pack_id: "freeform-test".to_string(),
        },
    )
    .await
    .unwrap();

    // 4. Propose a step
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "create allianz lux cbu".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::SentencePlayback { .. }
    ));

    // 5. Confirm
    let resp = orch
        .process(session_id, UserInputV2::Confirm)
        .await
        .unwrap();
    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::RunbookSummary { .. }
    ));

    // 6. Run (all sync)
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            },
        )
        .await
        .unwrap();

    match &resp.kind {
        ReplResponseKindV2::Executed { results } => {
            assert_eq!(results.len(), 1);
            assert!(results[0].success);
            assert_eq!(results[0].message.as_deref(), Some("Completed"));
        }
        other => panic!(
            "Expected Executed, got: {:?}",
            std::mem::discriminant(other)
        ),
    }

    // 7. Session back to RunbookEditing
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}

// ===========================================================================
// TEST 26: Golden loop with durable
// ===========================================================================

#[tokio::test]
async fn test_golden_loop_with_durable() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);

    // 1. Create session + scope + pack
    let session_id = orch.create_session().await;
    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Allianz".to_string(),
        },
    )
    .await
    .unwrap();
    orch.process(
        session_id,
        UserInputV2::SelectPack {
            pack_id: "freeform-test".to_string(),
        },
    )
    .await
    .unwrap();

    // 2. Propose and confirm
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "create allianz lux cbu".to_string(),
        },
    )
    .await
    .unwrap();
    orch.process(session_id, UserInputV2::Confirm)
        .await
        .unwrap();

    let entry_id = {
        let session = orch.get_session(session_id).await.unwrap();
        session.runbook.entries[0].id
    };

    // 3. Set to Durable with :park marker
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::Durable;
        session.runbook.entries[0].dsl = "(doc.solicit :park :entity-id \"test\")".to_string();
    }

    // 4. Run -> parks
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            },
        )
        .await
        .unwrap();
    assert!(
        resp.message.contains("parked") || resp.message.contains("Parked"),
        "Expected parked message, got: {}",
        resp.message
    );

    // 5. Verify parked state
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::Executing { .. }));
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Parked
    );

    // 6. Simulate external signal: resume the entry
    let correlation_key = {
        let session = orch.get_session(session_id).await.unwrap();
        session
            .runbook
            .entry_by_id(entry_id)
            .unwrap()
            .invocation
            .as_ref()
            .unwrap()
            .correlation_key
            .clone()
    };
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.resume_entry(
            &correlation_key,
            Some(serde_json::json!({"completed": true})),
        );
    }

    // 7. Continue execution
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Resume(entry_id),
        },
    )
    .await
    .unwrap();

    // 8. Verify completed
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Completed
    );
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}

// ===========================================================================
// TEST 27: Golden loop with human gate
// ===========================================================================

#[tokio::test]
async fn test_golden_loop_with_human_gate() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);

    // 1. Create session + scope + pack
    let session_id = orch.create_session().await;
    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Allianz".to_string(),
        },
    )
    .await
    .unwrap();
    orch.process(
        session_id,
        UserInputV2::SelectPack {
            pack_id: "freeform-test".to_string(),
        },
    )
    .await
    .unwrap();

    // 2. Propose and confirm
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "create allianz lux cbu".to_string(),
        },
    )
    .await
    .unwrap();
    orch.process(session_id, UserInputV2::Confirm)
        .await
        .unwrap();

    let entry_id = {
        let session = orch.get_session(session_id).await.unwrap();
        session.runbook.entries[0].id
    };

    // 3. Set to HumanGate
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session.runbook.entries[0].execution_mode = ExecutionMode::HumanGate;
    }

    // 4. Run -> parks (DSL not executed)
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // 5. Verify parked
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::Executing { .. }));
    let entry = session.runbook.entry_by_id(entry_id).unwrap();
    assert_eq!(entry.status, EntryStatus::Parked);
    assert_eq!(
        entry.invocation.as_ref().unwrap().gate_type,
        GateType::HumanApproval
    );

    // 6. Approve -> executes DSL and completes
    orch.process(
        session_id,
        UserInputV2::Approve {
            entry_id,
            approved_by: Some("admin".to_string()),
        },
    )
    .await
    .unwrap();

    // 7. Verify completed
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Completed
    );
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}

// ===========================================================================
// TEST 28: Golden loop mixed modes
// ===========================================================================

#[tokio::test]
async fn test_golden_loop_mixed_modes() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_parkable_executor(matcher);

    // 1. Create session + scope + pack
    let session_id = orch.create_session().await;
    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Allianz".to_string(),
        },
    )
    .await
    .unwrap();
    orch.process(
        session_id,
        UserInputV2::SelectPack {
            pack_id: "freeform-test".to_string(),
        },
    )
    .await
    .unwrap();

    // 2. Propose and confirm 3 entries
    for _ in 0..3 {
        orch.process(
            session_id,
            UserInputV2::Message {
                content: "create allianz fund".to_string(),
            },
        )
        .await
        .unwrap();
        orch.process(session_id, UserInputV2::Confirm)
            .await
            .unwrap();
    }

    let entry_ids: Vec<Uuid> = {
        let session = orch.get_session(session_id).await.unwrap();
        session.runbook.entries.iter().map(|e| e.id).collect()
    };
    assert_eq!(entry_ids.len(), 3);

    // 3. Set execution modes: Entry 1 Sync, Entry 2 HumanGate, Entry 3 Sync
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        // Entry 0 stays Sync (default)
        session.runbook.entries[1].execution_mode = ExecutionMode::HumanGate;
        // Entry 2 stays Sync (default)
    }

    // 4. Run -> Entry 1 completes, Entry 2 parks
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // 5. Verify intermediate state
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries[0].status, EntryStatus::Completed);
    assert_eq!(session.runbook.entries[1].status, EntryStatus::Parked);
    assert_eq!(session.runbook.entries[2].status, EntryStatus::Confirmed);
    assert!(matches!(session.state, ReplStateV2::Executing { .. }));

    // 6. Approve Entry 2 -> Entry 2 executes, Entry 3 executes
    orch.process(
        session_id,
        UserInputV2::Approve {
            entry_id: entry_ids[1],
            approved_by: Some("admin".to_string()),
        },
    )
    .await
    .unwrap();

    // 7. Verify all completed
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries[0].status, EntryStatus::Completed);
    assert_eq!(session.runbook.entries[1].status, EntryStatus::Completed);
    assert_eq!(session.runbook.entries[2].status, EntryStatus::Completed);

    // 8. Session back to RunbookEditing
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}
