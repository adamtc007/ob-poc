//! Phase 6 Integration Tests — Direct DSL Input + Pack Handoff + Regression Wrappers
//!
//! These tests verify Phase 6 functionality:
//!
//! Scenario B: Direct DSL Input (1-4):
//!  1. Direct DSL input transitions to SentencePlayback with "Execute:" prefix
//!  2. Direct DSL confirm adds runbook entry with correct DSL
//!  3. Direct DSL reject returns to InPack, no entry added
//!  4. Direct DSL bypasses pack verb filter
//!
//! Scenario F: Pack Handoff (5-9):
//!  5. All-success execution with handoff_target transitions to target pack
//!  6. Handoff carries forwarded_context with client_group_id and outcome IDs
//!  7. Target pack not found falls back to RunbookEditing (no crash)
//!  8. HandoffReceived event emitted in new runbook audit trail
//!  9. Forwarded outcomes contain only Completed entry IDs
//!
//! Regression Wrappers (10-15):
//! 10. A1: Golden loop regression
//! 11. C1: Disambiguation regression
//! 12. D1: Pack shorthand regression
//! 13. E1: Runbook editing regression
//! 14. G1: Durable park+resume regression
//! 15. H1: Force-select verb regression
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
use ob_poc::repl::orchestrator_v2::{ParkableStubExecutor, ReplOrchestratorV2, StubExecutor};
use ob_poc::repl::proposal_engine::ProposalEngine;
use ob_poc::repl::response_v2::ReplResponseKindV2;
use ob_poc::repl::runbook::{EntryStatus, ExecutionMode, RunbookEvent};
use ob_poc::repl::types::{IntentMatchResult, MatchContext, MatchOutcome};
use ob_poc::repl::types_v2::{ReplCommandV2, ReplStateV2, UserInputV2};
use ob_poc::repl::verb_config_index::VerbConfigIndex;

// ===========================================================================
// Helpers (replicated from Phase 5)
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

/// Build a pack with allowed_verbs restricted to session.load-cbu only.
fn build_restricted_pack() -> (Arc<PackManifest>, String) {
    let yaml = r#"
id: restricted-test
name: Restricted Test Pack
version: "1.0"
description: "Pack that only allows session.load-cbu"
invocation_phrases:
  - "restricted test"
required_context: []
optional_context: []
allowed_verbs:
  - "session.load-cbu"
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

/// Build a source pack with handoff_target pointing to "target-pack".
fn build_source_pack() -> (Arc<PackManifest>, String) {
    let yaml = r#"
id: source-pack
name: Source Pack
version: "1.0"
description: "Pack that hands off to target-pack"
invocation_phrases:
  - "source pack"
handoff_target: target-pack
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

/// Build the target pack that receives handoff.
fn build_target_pack() -> (Arc<PackManifest>, String) {
    let yaml = r#"
id: target-pack
name: Target Pack
version: "1.0"
description: "Receives handoff"
invocation_phrases:
  - "target pack"
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

/// Build a source pack whose handoff_target points to a nonexistent pack.
fn build_dangling_handoff_pack() -> (Arc<PackManifest>, String) {
    let yaml = r#"
id: dangling-pack
name: Dangling Pack
version: "1.0"
description: "Pack with handoff to nonexistent target"
invocation_phrases:
  - "dangling pack"
handoff_target: nonexistent-pack
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

// ---------------------------------------------------------------------------
// MockIntentMatcher
// ---------------------------------------------------------------------------

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

    /// Returns DirectDsl outcome for input that looks like raw DSL.
    fn direct_dsl(source: &str) -> Self {
        Self {
            result: IntentMatchResult {
                outcome: MatchOutcome::DirectDsl {
                    source: source.to_string(),
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

// ---------------------------------------------------------------------------
// Orchestrator builders
// ---------------------------------------------------------------------------

/// Build an orchestrator with freeform pack and StubExecutor.
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

/// Build an orchestrator with a restricted pack (allowed_verbs = ["session.load-cbu"]).
fn build_orchestrator_restricted(matcher: MockIntentMatcher) -> ReplOrchestratorV2 {
    let index = Arc::new(build_real_index());
    let intent_matcher: Arc<dyn IntentMatcher> = Arc::new(matcher.clone());
    let intent_service = Arc::new(IntentService::new(intent_matcher.clone(), index.clone()));
    let engine = Arc::new(ProposalEngine::new(intent_service.clone(), index.clone()));

    let (pack, hash) = build_restricted_pack();
    let router = PackRouter::new(vec![(pack, hash)]);

    ReplOrchestratorV2::new(router, Arc::new(StubExecutor))
        .with_verb_config_index(index)
        .with_intent_matcher(intent_matcher)
        .with_intent_service(intent_service)
        .with_proposal_engine(engine)
}

/// Build an orchestrator with source-pack + target-pack for handoff tests.
fn build_orchestrator_with_handoff(matcher: MockIntentMatcher) -> ReplOrchestratorV2 {
    let index = Arc::new(build_real_index());
    let intent_matcher: Arc<dyn IntentMatcher> = Arc::new(matcher.clone());
    let intent_service = Arc::new(IntentService::new(intent_matcher.clone(), index.clone()));
    let engine = Arc::new(ProposalEngine::new(intent_service.clone(), index.clone()));

    let (source, source_hash) = build_source_pack();
    let (target, target_hash) = build_target_pack();
    let router = PackRouter::new(vec![(source, source_hash), (target, target_hash)]);

    ReplOrchestratorV2::new(router, Arc::new(StubExecutor))
        .with_verb_config_index(index)
        .with_intent_matcher(intent_matcher)
        .with_intent_service(intent_service)
        .with_proposal_engine(engine)
}

/// Build an orchestrator with dangling handoff (target not in router).
fn build_orchestrator_dangling_handoff(matcher: MockIntentMatcher) -> ReplOrchestratorV2 {
    let index = Arc::new(build_real_index());
    let intent_matcher: Arc<dyn IntentMatcher> = Arc::new(matcher.clone());
    let intent_service = Arc::new(IntentService::new(intent_matcher.clone(), index.clone()));
    let engine = Arc::new(ProposalEngine::new(intent_service.clone(), index.clone()));

    // Only the dangling pack — no target-pack in router.
    let (dangling, dangling_hash) = build_dangling_handoff_pack();
    let router = PackRouter::new(vec![(dangling, dangling_hash)]);

    ReplOrchestratorV2::new(router, Arc::new(StubExecutor))
        .with_verb_config_index(index)
        .with_intent_matcher(intent_matcher)
        .with_intent_service(intent_service)
        .with_proposal_engine(engine)
}

/// Build an orchestrator with ParkableStubExecutor for durable tests.
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

// ---------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------

/// Scope + pack selection -> session lands in InPack for a given orchestrator
/// and pack_id.
async fn setup_in_pack_with_id(orch: &ReplOrchestratorV2, pack_id: &str) -> Uuid {
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

    // Select pack
    orch.process(
        session_id,
        UserInputV2::SelectPack {
            pack_id: pack_id.to_string(),
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

/// Scope + freeform pack selection -> InPack.
async fn setup_in_pack(orch: &ReplOrchestratorV2) -> Uuid {
    setup_in_pack_with_id(orch, "freeform-test").await
}

/// Set up InPack session, propose + confirm one entry, return (session_id, entry_id).
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

    assert!(
        matches!(resp.kind, ReplResponseKindV2::SentencePlayback { .. }),
        "Expected SentencePlayback, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );

    // Confirm
    let resp = orch
        .process(session_id, UserInputV2::Confirm)
        .await
        .unwrap();

    assert!(
        matches!(resp.kind, ReplResponseKindV2::RunbookSummary { .. }),
        "Expected RunbookSummary after confirm, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );

    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries.len(), 1);
    let entry_id = session.runbook.entries[0].id;

    (session_id, entry_id)
}

/// Set up InPack, propose + confirm one entry in a named pack.
async fn setup_with_one_entry_in_pack(orch: &ReplOrchestratorV2, pack_id: &str) -> (Uuid, Uuid) {
    let session_id = setup_in_pack_with_id(orch, pack_id).await;

    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "create allianz lux cbu".to_string(),
            },
        )
        .await
        .unwrap();

    assert!(
        matches!(resp.kind, ReplResponseKindV2::SentencePlayback { .. }),
        "Expected SentencePlayback, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );

    let resp = orch
        .process(session_id, UserInputV2::Confirm)
        .await
        .unwrap();

    assert!(
        matches!(resp.kind, ReplResponseKindV2::RunbookSummary { .. }),
        "Expected RunbookSummary after confirm, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );

    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries.len(), 1);
    let entry_id = session.runbook.entries[0].id;

    (session_id, entry_id)
}

/// Set up session with n confirmed entries in a named pack.
async fn setup_with_n_entries_in_pack(
    orch: &ReplOrchestratorV2,
    pack_id: &str,
    n: usize,
) -> (Uuid, Vec<Uuid>) {
    let session_id = setup_in_pack_with_id(orch, pack_id).await;
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

// ===========================================================================
// SCENARIO B: DIRECT DSL INPUT
// ===========================================================================

// ===========================================================================
// TEST B1: Direct DSL input transitions to SentencePlayback
// ===========================================================================

#[tokio::test]
async fn test_b1_direct_dsl_transitions_to_sentence_playback() {
    let dsl_input = "(cbu.create :name \"test\")";
    let matcher = MockIntentMatcher::direct_dsl(dsl_input);
    let orch = build_orchestrator_with_engine(matcher);
    let session_id = setup_in_pack(&orch).await;

    // Send the direct DSL as a message
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: dsl_input.to_string(),
            },
        )
        .await
        .unwrap();

    // Should be SentencePlayback
    match &resp.kind {
        ReplResponseKindV2::SentencePlayback { sentence, verb, .. } => {
            assert!(
                sentence.contains("Execute"),
                "Sentence should contain 'Execute', got: {}",
                sentence
            );
            // DirectDsl uses "direct.dsl" as the verb marker
            assert_eq!(verb, "direct.dsl");
        }
        other => panic!(
            "Expected SentencePlayback, got: {:?}",
            std::mem::discriminant(other)
        ),
    }

    // State should be SentencePlayback
    let session = orch.get_session(session_id).await.unwrap();
    assert!(
        matches!(session.state, ReplStateV2::SentencePlayback { .. }),
        "Expected SentencePlayback state, got: {:?}",
        std::mem::discriminant(&session.state)
    );
}

// ===========================================================================
// TEST B2: Direct DSL confirm adds runbook entry with correct DSL
// ===========================================================================

#[tokio::test]
async fn test_b2_direct_dsl_confirm_adds_entry() {
    let dsl_input = "(cbu.create :name \"test\")";
    let matcher = MockIntentMatcher::direct_dsl(dsl_input);
    let orch = build_orchestrator_with_engine(matcher);
    let session_id = setup_in_pack(&orch).await;

    // Send direct DSL
    orch.process(
        session_id,
        UserInputV2::Message {
            content: dsl_input.to_string(),
        },
    )
    .await
    .unwrap();

    // Confirm
    let resp = orch
        .process(session_id, UserInputV2::Confirm)
        .await
        .unwrap();

    assert!(
        matches!(resp.kind, ReplResponseKindV2::RunbookSummary { .. }),
        "Expected RunbookSummary after confirm, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );

    // Verify entry was added with correct DSL
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries.len(), 1);
    let entry = &session.runbook.entries[0];
    assert!(
        entry.dsl.contains("cbu.create"),
        "Entry DSL should contain cbu.create, got: {}",
        entry.dsl
    );
    assert_eq!(entry.status, EntryStatus::Confirmed);
}

// ===========================================================================
// TEST B3: Direct DSL reject returns to InPack, no entry added
// ===========================================================================

#[tokio::test]
async fn test_b3_direct_dsl_reject_returns_to_in_pack() {
    let dsl_input = "(cbu.create :name \"test\")";
    let matcher = MockIntentMatcher::direct_dsl(dsl_input);
    let orch = build_orchestrator_with_engine(matcher);
    let session_id = setup_in_pack(&orch).await;

    // Send direct DSL
    orch.process(
        session_id,
        UserInputV2::Message {
            content: dsl_input.to_string(),
        },
    )
    .await
    .unwrap();

    // Reject
    let resp = orch.process(session_id, UserInputV2::Reject).await.unwrap();

    // Should be back in InPack
    let session = orch.get_session(session_id).await.unwrap();
    assert!(
        matches!(session.state, ReplStateV2::InPack { .. }),
        "After reject, session should be InPack, got: {:?}",
        std::mem::discriminant(&session.state)
    );

    // No entries added
    assert_eq!(
        session.runbook.entries.len(),
        0,
        "Runbook should have no entries after reject, got: {}",
        session.runbook.entries.len()
    );

    // Response should indicate rejection
    assert!(
        resp.message.to_lowercase().contains("reject")
            || resp.message.to_lowercase().contains("discard")
            || resp.message.to_lowercase().contains("cancel"),
        "Response should mention rejection, got: {}",
        resp.message
    );
}

// ===========================================================================
// TEST B4: Direct DSL bypasses pack verb filter
// ===========================================================================

#[tokio::test]
async fn test_b4_direct_dsl_bypasses_pack_verb_filter() {
    // The restricted pack only allows session.load-cbu.
    // DirectDsl with cbu.create should still reach SentencePlayback.
    let dsl_input = "(cbu.create :name \"test\")";
    let matcher = MockIntentMatcher::direct_dsl(dsl_input);
    let orch = build_orchestrator_restricted(matcher);
    let session_id = setup_in_pack_with_id(&orch, "restricted-test").await;

    // Send direct DSL for a verb NOT in allowed_verbs
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: dsl_input.to_string(),
            },
        )
        .await
        .unwrap();

    // Should still reach SentencePlayback because DirectDsl bypasses filter
    match &resp.kind {
        ReplResponseKindV2::SentencePlayback { sentence, verb, .. } => {
            assert!(
                sentence.contains("Execute"),
                "Sentence should contain 'Execute', got: {}",
                sentence
            );
            // DirectDsl uses "direct.dsl" as the verb marker
            assert_eq!(verb, "direct.dsl");
        }
        other => panic!(
            "Expected SentencePlayback (bypass filter), got: {:?}",
            std::mem::discriminant(other)
        ),
    }
}

// ===========================================================================
// SCENARIO F: PACK HANDOFF
// ===========================================================================

// ===========================================================================
// TEST F1: All-success execution with handoff_target transitions to target pack
// ===========================================================================

#[tokio::test]
async fn test_f1_handoff_transitions_to_target_pack() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_handoff(matcher);
    let (session_id, _entry_id) = setup_with_one_entry_in_pack(&orch, "source-pack").await;

    // Run -> all success -> handoff should trigger
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            },
        )
        .await
        .unwrap();

    // Response should mention handoff to target pack
    assert!(
        resp.message.contains("Target Pack") || resp.message.contains("target-pack"),
        "Expected handoff message mentioning target pack, got: {}",
        resp.message
    );

    // Session should now be in InPack for target-pack
    let session = orch.get_session(session_id).await.unwrap();
    match &session.state {
        ReplStateV2::InPack { pack_id, .. } => {
            assert_eq!(
                pack_id, "target-pack",
                "Expected InPack for target-pack, got: {}",
                pack_id
            );
        }
        other => panic!(
            "Expected InPack state for target-pack, got: {:?}",
            std::mem::discriminant(other)
        ),
    }

    // Journey context should point to target pack
    assert!(session.journey_context.is_some());
    assert_eq!(
        session.journey_context.as_ref().unwrap().pack.id,
        "target-pack"
    );
}

// ===========================================================================
// TEST F2: Handoff carries forwarded_context with client_group_id and outcome IDs
// ===========================================================================

#[tokio::test]
async fn test_f2_handoff_carries_forwarded_context() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_handoff(matcher);

    // Set up session with scope (so client_context is populated)
    let session_id = orch.create_session().await;
    let client_group_id = Uuid::new_v4();

    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: client_group_id,
            group_name: "Allianz".to_string(),
        },
    )
    .await
    .unwrap();

    orch.process(
        session_id,
        UserInputV2::SelectPack {
            pack_id: "source-pack".to_string(),
        },
    )
    .await
    .unwrap();

    // Add and confirm an entry
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

    orch.process(session_id, UserInputV2::Confirm)
        .await
        .unwrap();

    let entry_id = {
        let session = orch.get_session(session_id).await.unwrap();
        session.runbook.entries[0].id
    };

    // Run -> triggers handoff
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // Check HandoffReceived event in new runbook's audit trail
    let session = orch.get_session(session_id).await.unwrap();
    let handoff_event = session
        .runbook
        .audit
        .iter()
        .find(|e| matches!(e, RunbookEvent::HandoffReceived { .. }));

    assert!(
        handoff_event.is_some(),
        "Expected HandoffReceived event in audit trail"
    );

    match handoff_event.unwrap() {
        RunbookEvent::HandoffReceived {
            forwarded_context, ..
        } => {
            // Should contain client_group_id
            assert!(
                forwarded_context.contains_key("client_group_id"),
                "forwarded_context should contain client_group_id, keys: {:?}",
                forwarded_context.keys().collect::<Vec<_>>()
            );
            assert_eq!(
                forwarded_context.get("client_group_id").unwrap(),
                &client_group_id.to_string()
            );

            // Should contain outcome entry IDs
            assert!(
                forwarded_context.contains_key("outcome_0"),
                "forwarded_context should contain outcome_0, keys: {:?}",
                forwarded_context.keys().collect::<Vec<_>>()
            );
            assert_eq!(
                forwarded_context.get("outcome_0").unwrap(),
                &entry_id.to_string()
            );
        }
        _ => unreachable!(),
    }
}

// ===========================================================================
// TEST F3: Target pack not found -> graceful fallback to RunbookEditing
// ===========================================================================

#[tokio::test]
async fn test_f3_target_not_found_falls_back_to_runbook_editing() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_dangling_handoff(matcher);
    let (session_id, _entry_id) = setup_with_one_entry_in_pack(&orch, "dangling-pack").await;

    // Run -> all success -> handoff target not found -> should NOT crash
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            },
        )
        .await
        .unwrap();

    // Should complete normally (no crash), back to RunbookEditing
    let session = orch.get_session(session_id).await.unwrap();
    assert!(
        matches!(session.state, ReplStateV2::RunbookEditing),
        "Expected RunbookEditing after failed handoff, got: {:?}",
        std::mem::discriminant(&session.state)
    );

    // Execution results should still be returned
    match &resp.kind {
        ReplResponseKindV2::Executed { results } => {
            assert_eq!(results.len(), 1);
            assert!(results[0].success);
        }
        other => panic!(
            "Expected Executed response, got: {:?}",
            std::mem::discriminant(other)
        ),
    }
}

// ===========================================================================
// TEST F4: HandoffReceived event emitted in target runbook audit trail
// ===========================================================================

#[tokio::test]
async fn test_f4_handoff_received_event_emitted() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_handoff(matcher);
    let (session_id, _entry_id) = setup_with_one_entry_in_pack(&orch, "source-pack").await;

    // Capture source runbook ID before execution
    let source_runbook_id = {
        let session = orch.get_session(session_id).await.unwrap();
        session.runbook.id
    };

    // Run -> triggers handoff
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // The new runbook (target pack) should have HandoffReceived event
    let session = orch.get_session(session_id).await.unwrap();

    // Runbook should be a NEW runbook (different ID from source)
    assert_ne!(
        session.runbook.id, source_runbook_id,
        "Target runbook should have a new ID"
    );

    // Check HandoffReceived event
    let handoff_events: Vec<_> = session
        .runbook
        .audit
        .iter()
        .filter(|e| matches!(e, RunbookEvent::HandoffReceived { .. }))
        .collect();

    assert_eq!(
        handoff_events.len(),
        1,
        "Expected exactly 1 HandoffReceived event, got: {}",
        handoff_events.len()
    );

    match handoff_events[0] {
        RunbookEvent::HandoffReceived {
            source_runbook_id: src_id,
            target_pack_id,
            ..
        } => {
            assert_eq!(*src_id, source_runbook_id);
            assert_eq!(target_pack_id, "target-pack");
        }
        _ => unreachable!(),
    }
}

// ===========================================================================
// TEST F5: Forwarded outcomes contain only Completed entry IDs
// ===========================================================================

#[tokio::test]
async fn test_f5_forwarded_outcomes_only_completed() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_handoff(matcher);
    let (session_id, entry_ids) = setup_with_n_entries_in_pack(&orch, "source-pack", 3).await;

    // Disable entry 1 (index 1) so it gets skipped during execution
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Disable(entry_ids[1]),
        },
    )
    .await
    .unwrap();

    // Verify entry 1 is disabled
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries[1].status, EntryStatus::Disabled);

    // Run -> entries 0 and 2 execute (1 is disabled), all success -> handoff
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    // Check HandoffReceived event for forwarded outcomes
    let session = orch.get_session(session_id).await.unwrap();
    let handoff_event = session
        .runbook
        .audit
        .iter()
        .find(|e| matches!(e, RunbookEvent::HandoffReceived { .. }));

    assert!(handoff_event.is_some(), "Expected HandoffReceived event");

    match handoff_event.unwrap() {
        RunbookEvent::HandoffReceived {
            forwarded_context, ..
        } => {
            // Should have outcome_0 and outcome_1 (the two completed entries)
            // but NOT include the disabled entry's ID.
            let outcome_ids: Vec<String> = forwarded_context
                .iter()
                .filter(|(k, _)| k.starts_with("outcome_"))
                .map(|(_, v)| v.clone())
                .collect();

            // Only completed entries should be forwarded
            assert!(
                outcome_ids.contains(&entry_ids[0].to_string()),
                "Forwarded outcomes should contain entry 0"
            );
            assert!(
                outcome_ids.contains(&entry_ids[2].to_string()),
                "Forwarded outcomes should contain entry 2"
            );
            assert!(
                !outcome_ids.contains(&entry_ids[1].to_string()),
                "Forwarded outcomes should NOT contain disabled entry 1"
            );
        }
        _ => unreachable!(),
    }
}

// ===========================================================================
// REGRESSION WRAPPERS
// ===========================================================================

// ===========================================================================
// TEST A1: Golden loop regression
// ===========================================================================

#[tokio::test]
async fn test_a1_golden_loop_regression() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);

    // Create session
    let session_id = orch.create_session().await;

    // Scope
    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Allianz".to_string(),
        },
    )
    .await
    .unwrap();

    // Pack
    orch.process(
        session_id,
        UserInputV2::SelectPack {
            pack_id: "freeform-test".to_string(),
        },
    )
    .await
    .unwrap();

    // Propose
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

    // Confirm
    orch.process(session_id, UserInputV2::Confirm)
        .await
        .unwrap();

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

    match &resp.kind {
        ReplResponseKindV2::Executed { results } => {
            assert_eq!(results.len(), 1);
            assert!(results[0].success);
        }
        other => panic!(
            "Expected Executed, got: {:?}",
            std::mem::discriminant(other)
        ),
    }

    // Back to RunbookEditing
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}

// ===========================================================================
// TEST C1: Disambiguation regression
// ===========================================================================

#[tokio::test]
async fn test_c1_disambiguation_regression() {
    let matcher = MockIntentMatcher::ambiguous(
        vec![("session.load-galaxy", 0.82), ("session.load-cbu", 0.79)],
        0.03,
    );
    let orch = build_orchestrator_with_engine(matcher);
    let session_id = setup_in_pack(&orch).await;

    // Send ambiguous message
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "load the book".to_string(),
            },
        )
        .await
        .unwrap();

    // Ambiguous matches via ProposalEngine return StepProposals with multiple options
    match &resp.kind {
        ReplResponseKindV2::StepProposals { proposals, .. } => {
            assert!(
                proposals.len() >= 2,
                "Expected at least 2 proposals for ambiguous match, got: {}",
                proposals.len()
            );
        }
        other => panic!(
            "Expected StepProposals for ambiguous match, got: {:?}",
            std::mem::discriminant(other)
        ),
    }
}

// ===========================================================================
// TEST D1: Pack shorthand regression (SelectPack -> InPack)
// ===========================================================================

#[tokio::test]
async fn test_d1_pack_shorthand_regression() {
    let matcher = MockIntentMatcher::matched("cbu.create", 0.92, None);
    let orch = build_orchestrator_with_engine(matcher);

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

    // Select pack directly
    let resp = orch
        .process(
            session_id,
            UserInputV2::SelectPack {
                pack_id: "freeform-test".to_string(),
            },
        )
        .await
        .unwrap();

    // Should land in InPack
    let session = orch.get_session(session_id).await.unwrap();
    assert!(
        matches!(session.state, ReplStateV2::InPack { ref pack_id, .. } if pack_id == "freeform-test"),
        "Expected InPack for freeform-test, got: {:?}",
        std::mem::discriminant(&session.state)
    );

    // Response should acknowledge pack activation
    assert!(
        resp.message.contains("Pack")
            || resp.message.contains("pack")
            || resp.message.contains("activated"),
        "Response should acknowledge pack activation, got: {}",
        resp.message
    );
}

// ===========================================================================
// TEST E1: Runbook editing regression (edit entry args)
// ===========================================================================

#[tokio::test]
async fn test_e1_runbook_editing_regression() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Edit the name field
    let resp = orch
        .process(
            session_id,
            UserInputV2::Edit {
                step_id: entry_id,
                field: "name".to_string(),
                value: "Aviva IE".to_string(),
            },
        )
        .await
        .unwrap();

    // Should succeed (not error)
    assert!(
        !matches!(
            resp.kind,
            ReplResponseKindV2::Error {
                recoverable: false,
                ..
            }
        ),
        "Edit should not return a fatal error, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );

    // Verify entry was updated
    let session = orch.get_session(session_id).await.unwrap();
    let entry = session.runbook.entry_by_id(entry_id).unwrap();
    assert!(
        entry
            .args
            .get("name")
            .map(|v| v.contains("Aviva"))
            .unwrap_or(false)
            || entry.dsl.contains("Aviva"),
        "Entry should reflect the edit to 'Aviva IE'"
    );
}

// ===========================================================================
// TEST G1: Durable park+resume regression
// ===========================================================================

#[tokio::test]
async fn test_g1_durable_park_resume_regression() {
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
        session.runbook.entries[0].dsl = "(doc.solicit :park :entity-id \"test\")".to_string();
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
    assert!(matches!(session.state, ReplStateV2::Executing { .. }));

    // Simulate external signal
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
        session
            .runbook
            .resume_entry(&correlation_key, Some(serde_json::json!({"ok": true})));
    }

    // Resume
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Resume(entry_id),
        },
    )
    .await
    .unwrap();

    // Verify completed
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Completed
    );
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}

// ===========================================================================
// TEST H1: Force-select verb regression
// ===========================================================================

#[tokio::test]
async fn test_h1_force_select_verb_regression() {
    // Start with ambiguous matcher
    let matcher = MockIntentMatcher::ambiguous(
        vec![("session.load-galaxy", 0.82), ("session.load-cbu", 0.79)],
        0.03,
    );
    let orch = build_orchestrator_with_engine(matcher);
    let session_id = setup_in_pack(&orch).await;

    // Send ambiguous message -> StepProposals
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "load the book".to_string(),
            },
        )
        .await
        .unwrap();

    // Via ProposalEngine, ambiguous returns StepProposals
    let proposal_id = match &resp.kind {
        ReplResponseKindV2::StepProposals { proposals, .. } => {
            assert!(proposals.len() >= 2, "Expected multiple proposals");
            // Pick the first proposal (session.load-galaxy)
            let galaxy_proposal = proposals
                .iter()
                .find(|p| p.verb == "session.load-galaxy")
                .unwrap_or(&proposals[0]);
            galaxy_proposal.id
        }
        other => panic!(
            "Expected StepProposals, got: {:?}",
            std::mem::discriminant(other)
        ),
    };

    // Select the proposal
    let resp = orch
        .process(session_id, UserInputV2::SelectProposal { proposal_id })
        .await
        .unwrap();

    // Should advance to SentencePlayback (proposal selected)
    let session = orch.get_session(session_id).await.unwrap();
    assert!(
        matches!(session.state, ReplStateV2::SentencePlayback { .. }),
        "After SelectProposal, should be SentencePlayback, got: {:?}",
        std::mem::discriminant(&session.state)
    );

    // Response should show the selected verb's sentence
    match &resp.kind {
        ReplResponseKindV2::SentencePlayback { verb, .. } => {
            assert_eq!(verb, "session.load-galaxy");
        }
        other => panic!(
            "Expected SentencePlayback after SelectProposal, got: {:?}",
            std::mem::discriminant(other)
        ),
    }
}
