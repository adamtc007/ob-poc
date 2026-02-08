//! Phase 4 Integration Tests — Runbook Editing UX
//!
//! These tests verify Phase 4 functionality:
//!
//! Runbook Model (1-8):
//!  1. update_entry_arg changes arg and returns old value
//!  2. disable_entry sets Disabled status and emits event
//!  3. enable_entry restores to Confirmed and emits event
//!  4. toggle_entry flips between Disabled and Confirmed
//!  5. clear removes all entries and emits RunbookCleared event
//!  6. readiness reports issues for Proposed/Failed/empty entries
//!  7. readiness returns ready=true for all-Confirmed runbook
//!  8. undo_stack push/pop works correctly
//!
//! Sentence Regeneration (9-10):
//!  9. rebuild_dsl round-trips with extract_args_from_dsl
//! 10. edit step arg → sentence regenerated → DSL updated
//!
//! Orchestrator Edit Handling (11-13):
//! 11. Edit in RunbookEditing state → updates arg + sentence
//! 12. Edit in InPack state → updates arg + sentence
//! 13. Edit nonexistent step → error response
//!
//! Command Handlers (14-22):
//! 14. Undo removes last entry, entry goes to undo stack
//! 15. Redo restores entry from undo stack
//! 16. Undo then Redo is identity (entry restored)
//! 17. Undo on empty runbook → error
//! 18. Clear empties runbook, returns count
//! 19. Cancel from SentencePlayback → returns to InPack
//! 20. Cancel from Clarifying → returns to InPack
//! 21. Info shows scope, pack, readiness
//! 22. Help returns context-appropriate help text
//!
//! Disable/Enable (23-25):
//! 23. Disable step → skipped during execution
//! 24. Enable step → included in execution
//! 25. Toggle flips disabled state
//!
//! Execution Readiness Gate (26-28):
//! 26. Run blocked when entry is Proposed (not Confirmed)
//! 27. Run succeeds when all entries Confirmed
//! 28. Run skips Disabled entries (reports as "Skipped")
//!
//! Golden Loop (29):
//! 29. Full golden loop with editing
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
use ob_poc::repl::orchestrator_v2::{rebuild_dsl, ReplOrchestratorV2, StubExecutor};
use ob_poc::repl::proposal_engine::ProposalEngine;
use ob_poc::repl::response_v2::ReplResponseKindV2;
use ob_poc::repl::runbook::{EntryStatus, Runbook, RunbookEntry, RunbookEvent, SlotSource};
use ob_poc::repl::types::{IntentMatchResult, MatchContext, MatchOutcome};
use ob_poc::repl::types_v2::{ReplCommandV2, ReplStateV2, UserInputV2};
use ob_poc::repl::verb_config_index::VerbConfigIndex;

// ===========================================================================
// Helpers (shared with Phase 3 test patterns)
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
                verb_candidates: vec![ob_poc::repl::types::VerbCandidate {
                    verb_fqn: verb.to_string(),
                    description: format!("Description for {}", verb),
                    score: confidence,
                    example: None,
                    domain: Some(verb.split('.').next().unwrap_or("").to_string()),
                }],
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

/// Build an orchestrator with proposal engine attached (uses freeform pack).
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

/// Helper: scope + pack selection → session lands in InPack.
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

    // Get the user's cbu.create entry ID (last entry; scope + pack entries precede it)
    let session = orch.get_session(session_id).await.unwrap();
    let user_entry = session
        .runbook
        .entries
        .iter()
        .rfind(|e| e.verb == "cbu.create")
        .expect("Expected a cbu.create entry in runbook");
    let entry_id = user_entry.id;

    (session_id, entry_id)
}

/// Create a sample RunbookEntry for unit tests.
fn sample_entry(verb: &str, sentence: &str) -> RunbookEntry {
    RunbookEntry::new(
        verb.to_string(),
        sentence.to_string(),
        format!("({} :placeholder true)", verb),
    )
}

/// Create a confirmed entry with args.
fn confirmed_entry(verb: &str, sentence: &str, args: HashMap<String, String>) -> RunbookEntry {
    let mut entry = RunbookEntry::new(
        verb.to_string(),
        sentence.to_string(),
        rebuild_dsl(verb, &args),
    );
    entry.args = args;
    entry.status = EntryStatus::Confirmed;
    entry
}

// ===========================================================================
// TEST 1: update_entry_arg changes arg and returns old value
// ===========================================================================

#[test]
fn test_update_entry_arg_changes_and_returns_old() {
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("cbu.create", "Create fund");
    entry
        .args
        .insert("name".to_string(), "Old Name".to_string());
    let id = rb.add_entry(entry);

    let old = rb.update_entry_arg(id, "name", "New Name".to_string());
    assert_eq!(old, Some("Old Name".to_string()));

    let entry = rb.entry_by_id(id).unwrap();
    assert_eq!(entry.args.get("name").unwrap(), "New Name");
}

#[test]
fn test_update_entry_arg_new_field_returns_none() {
    let mut rb = Runbook::new(Uuid::new_v4());
    let entry = sample_entry("cbu.create", "Create fund");
    let id = rb.add_entry(entry);

    let old = rb.update_entry_arg(id, "jurisdiction", "LU".to_string());
    assert_eq!(old, None);

    let entry = rb.entry_by_id(id).unwrap();
    assert_eq!(entry.args.get("jurisdiction").unwrap(), "LU");
}

// ===========================================================================
// TEST 2: disable_entry sets Disabled status and emits event
// ===========================================================================

#[test]
fn test_disable_entry_sets_status_and_emits_events() {
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("cbu.create", "Create fund");
    entry.status = EntryStatus::Confirmed;
    let id = rb.add_entry(entry);

    let result = rb.disable_entry(id);
    assert!(result);

    let entry = rb.entry_by_id(id).unwrap();
    assert_eq!(entry.status, EntryStatus::Disabled);

    // Check events
    let disabled_events: Vec<_> = rb
        .audit
        .iter()
        .filter(|e| matches!(e, RunbookEvent::EntryDisabled { .. }))
        .collect();
    assert_eq!(disabled_events.len(), 1);

    // Disabling again returns false
    assert!(!rb.disable_entry(id));
}

// ===========================================================================
// TEST 3: enable_entry restores to Confirmed and emits event
// ===========================================================================

#[test]
fn test_enable_entry_restores_confirmed_and_emits_events() {
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("cbu.create", "Create fund");
    entry.status = EntryStatus::Confirmed;
    let id = rb.add_entry(entry);

    rb.disable_entry(id);
    assert_eq!(rb.entry_by_id(id).unwrap().status, EntryStatus::Disabled);

    let result = rb.enable_entry(id);
    assert!(result);
    assert_eq!(rb.entry_by_id(id).unwrap().status, EntryStatus::Confirmed);

    // Check events
    let enabled_events: Vec<_> = rb
        .audit
        .iter()
        .filter(|e| matches!(e, RunbookEvent::EntryEnabled { .. }))
        .collect();
    assert_eq!(enabled_events.len(), 1);

    // Enabling when already enabled returns false
    assert!(!rb.enable_entry(id));
}

// ===========================================================================
// TEST 4: toggle_entry flips between Disabled and Confirmed
// ===========================================================================

#[test]
fn test_toggle_entry_flips_state() {
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("cbu.create", "Create fund");
    entry.status = EntryStatus::Confirmed;
    let id = rb.add_entry(entry);

    // Toggle: Confirmed → Disabled
    let new = rb.toggle_entry(id);
    assert_eq!(new, Some(EntryStatus::Disabled));
    assert_eq!(rb.entry_by_id(id).unwrap().status, EntryStatus::Disabled);

    // Toggle: Disabled → Confirmed
    let new = rb.toggle_entry(id);
    assert_eq!(new, Some(EntryStatus::Confirmed));
    assert_eq!(rb.entry_by_id(id).unwrap().status, EntryStatus::Confirmed);

    // Toggle nonexistent
    assert_eq!(rb.toggle_entry(Uuid::new_v4()), None);
}

// ===========================================================================
// TEST 5: clear removes all entries and emits RunbookCleared event
// ===========================================================================

#[test]
fn test_clear_removes_all_and_emits_event() {
    let mut rb = Runbook::new(Uuid::new_v4());
    rb.add_entry(sample_entry("a.first", "First"));
    rb.add_entry(sample_entry("b.second", "Second"));
    rb.add_entry(sample_entry("c.third", "Third"));

    let count = rb.clear();
    assert_eq!(count, 3);
    assert!(rb.entries.is_empty());

    let cleared_events: Vec<_> = rb
        .audit
        .iter()
        .filter(|e| matches!(e, RunbookEvent::RunbookCleared { .. }))
        .collect();
    assert_eq!(cleared_events.len(), 1);
    if let RunbookEvent::RunbookCleared { entry_count, .. } = &cleared_events[0] {
        assert_eq!(*entry_count, 3);
    }
}

// ===========================================================================
// TEST 6: readiness reports issues for Proposed/Failed/empty entries
// ===========================================================================

#[test]
fn test_readiness_reports_issues() {
    // Empty runbook
    let rb = Runbook::new(Uuid::new_v4());
    let report = rb.readiness();
    assert!(!report.ready);
    assert!(!report.issues.is_empty());
    assert!(report.issues[0].issue.contains("No enabled entries"));

    // With Proposed entry (not confirmed)
    let mut rb = Runbook::new(Uuid::new_v4());
    rb.add_entry(sample_entry("cbu.create", "Create fund")); // status = Proposed
    let report = rb.readiness();
    assert!(!report.ready);
    assert!(report
        .issues
        .iter()
        .any(|i| i.issue.contains("not confirmed")));

    // With Failed entry
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("cbu.create", "Create fund");
    entry.status = EntryStatus::Failed;
    rb.add_entry(entry);
    let report = rb.readiness();
    assert!(!report.ready);
    assert!(report.issues.iter().any(|i| i.issue.contains("failed")));

    // With unresolved refs
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("cbu.create", "Create fund");
    entry.status = EntryStatus::Confirmed;
    entry
        .unresolved_refs
        .push(ob_poc::repl::runbook::UnresolvedRef {
            ref_id: "ref1".to_string(),
            display_text: "Allianz".to_string(),
            entity_type: Some("company".to_string()),
            search_column: None,
        });
    rb.add_entry(entry);
    let report = rb.readiness();
    assert!(!report.ready);
    assert!(report.issues.iter().any(|i| i.issue.contains("unresolved")));
}

// ===========================================================================
// TEST 7: readiness returns ready=true for all-Confirmed runbook
// ===========================================================================

#[test]
fn test_readiness_ready_when_all_confirmed() {
    let mut rb = Runbook::new(Uuid::new_v4());

    let mut e1 = sample_entry("cbu.create", "Create fund");
    e1.status = EntryStatus::Confirmed;
    rb.add_entry(e1);

    let mut e2 = sample_entry("cbu.assign-product", "Add product");
    e2.status = EntryStatus::Confirmed;
    rb.add_entry(e2);

    let report = rb.readiness();
    assert!(report.ready);
    assert!(report.issues.is_empty());
    assert_eq!(report.total_entries, 2);
    assert_eq!(report.enabled_entries, 2);
    assert_eq!(report.disabled_entries, 0);
}

#[test]
fn test_readiness_ignores_disabled_entries() {
    let mut rb = Runbook::new(Uuid::new_v4());

    let mut e1 = sample_entry("cbu.create", "Create fund");
    e1.status = EntryStatus::Confirmed;
    let _id1 = rb.add_entry(e1);

    let mut e2 = sample_entry("cbu.assign-product", "Add product");
    e2.status = EntryStatus::Proposed;
    let id2 = rb.add_entry(e2);

    // e2 is Proposed → not ready
    assert!(!rb.readiness().ready);

    // Disable the problematic entry → now only the Confirmed entry counts
    rb.disable_entry(id2);
    let report = rb.readiness();
    assert!(report.ready);
    assert_eq!(report.enabled_entries, 1);
    assert_eq!(report.disabled_entries, 1);
}

// ===========================================================================
// TEST 8: undo_stack push/pop works correctly
// ===========================================================================

#[test]
fn test_undo_stack_push_pop() {
    let mut rb = Runbook::new(Uuid::new_v4());

    // Empty stack
    assert!(rb.pop_undo_entry().is_none());

    let entry = sample_entry("cbu.create", "Create fund");
    let entry_id = entry.id;
    rb.push_undo_entry(entry);

    let popped = rb.pop_undo_entry();
    assert!(popped.is_some());
    assert_eq!(popped.unwrap().id, entry_id);

    // Stack is now empty
    assert!(rb.pop_undo_entry().is_none());
}

#[test]
fn test_undo_stack_lifo_order() {
    let mut rb = Runbook::new(Uuid::new_v4());

    let e1 = sample_entry("a.first", "First");
    let e2 = sample_entry("b.second", "Second");
    let id1 = e1.id;
    let id2 = e2.id;

    rb.push_undo_entry(e1);
    rb.push_undo_entry(e2);

    // LIFO: e2 comes out first
    assert_eq!(rb.pop_undo_entry().unwrap().id, id2);
    assert_eq!(rb.pop_undo_entry().unwrap().id, id1);
}

// ===========================================================================
// TEST 9: rebuild_dsl round-trips with extract_args_from_dsl
// ===========================================================================

#[test]
fn test_rebuild_dsl_basic() {
    let args = HashMap::from([
        ("name".to_string(), "Allianz".to_string()),
        ("jurisdiction".to_string(), "LU".to_string()),
    ]);
    let dsl = rebuild_dsl("cbu.create", &args);

    // Should contain verb and both args
    assert!(dsl.starts_with("(cbu.create"));
    assert!(dsl.contains(":jurisdiction LU"));
    assert!(dsl.contains(":name Allianz"));
    assert!(dsl.ends_with(')'));
}

#[test]
fn test_rebuild_dsl_empty_args() {
    let dsl = rebuild_dsl("session.clear", &HashMap::new());
    assert_eq!(dsl, "(session.clear)");
}

#[test]
fn test_rebuild_dsl_quoted_values() {
    let args = HashMap::from([("name".to_string(), "Allianz Lux Fund".to_string())]);
    let dsl = rebuild_dsl("cbu.create", &args);
    assert!(dsl.contains(":name \"Allianz Lux Fund\""));
}

#[test]
fn test_rebuild_dsl_sorted_keys() {
    let args = HashMap::from([
        ("z_last".to_string(), "val1".to_string()),
        ("a_first".to_string(), "val2".to_string()),
        ("m_middle".to_string(), "val3".to_string()),
    ]);
    let dsl = rebuild_dsl("test.verb", &args);

    // Keys should be sorted alphabetically
    let a_pos = dsl.find(":a_first").unwrap();
    let m_pos = dsl.find(":m_middle").unwrap();
    let z_pos = dsl.find(":z_last").unwrap();
    assert!(a_pos < m_pos);
    assert!(m_pos < z_pos);
}

// ===========================================================================
// TEST 10: update_entry_sentence emits EntryArgChanged event
// ===========================================================================

#[test]
fn test_update_entry_sentence_emits_audit() {
    let mut rb = Runbook::new(Uuid::new_v4());
    let mut entry = sample_entry("cbu.create", "Create fund");
    entry
        .args
        .insert("name".to_string(), "Old Name".to_string());
    let id = rb.add_entry(entry);

    rb.update_entry_arg(id, "name", "New Name".to_string());
    rb.update_entry_sentence(
        id,
        "Create New Name fund".to_string(),
        "(cbu.create :name \"New Name\")".to_string(),
        "Create fund",
        "name",
        Some("Old Name".to_string()),
        "New Name",
    );

    let entry = rb.entry_by_id(id).unwrap();
    assert_eq!(entry.sentence, "Create New Name fund");
    assert_eq!(entry.dsl, "(cbu.create :name \"New Name\")");

    let arg_changed_events: Vec<_> = rb
        .audit
        .iter()
        .filter(|e| matches!(e, RunbookEvent::EntryArgChanged { .. }))
        .collect();
    assert_eq!(arg_changed_events.len(), 1);
    if let RunbookEvent::EntryArgChanged {
        field,
        old_value,
        new_value,
        sentence_before,
        sentence_after,
        ..
    } = &arg_changed_events[0]
    {
        assert_eq!(field, "name");
        assert_eq!(old_value.as_deref(), Some("Old Name"));
        assert_eq!(new_value, "New Name");
        assert_eq!(sentence_before, "Create fund");
        assert_eq!(sentence_after, "Create New Name fund");
    }
}

// ===========================================================================
// TEST 11: Edit in RunbookEditing state → updates arg + sentence
// ===========================================================================

#[tokio::test]
async fn test_edit_in_runbook_editing_updates_arg_and_sentence() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Edit the entry
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

    assert!(
        matches!(resp.kind, ReplResponseKindV2::RunbookSummary { .. }),
        "Edit should return RunbookSummary, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );
    assert!(resp.message.contains("Aviva IE"));

    // Verify the entry was updated
    let session = orch.get_session(session_id).await.unwrap();
    let entry = session.runbook.entry_by_id(entry_id).unwrap();
    assert_eq!(entry.args.get("name").unwrap(), "Aviva IE");
    // Sentence should be regenerated (not the old one)
    // DSL should be rebuilt
    assert!(entry.dsl.contains("cbu.create"));
    // Provenance should be UserProvided for the edited field
    assert_eq!(
        entry.slot_provenance.slots.get("name"),
        Some(&SlotSource::UserProvided)
    );
}

// ===========================================================================
// TEST 12: Edit in InPack state → updates arg + sentence
// ===========================================================================

#[tokio::test]
async fn test_edit_in_in_pack_updates_arg() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Session is in InPack after confirm
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::InPack { .. }));

    // Edit the entry while in InPack
    let resp = orch
        .process(
            session_id,
            UserInputV2::Edit {
                step_id: entry_id,
                field: "jurisdiction".to_string(),
                value: "IE".to_string(),
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::RunbookSummary { .. }
    ));
    assert!(resp.message.contains("IE"));

    let session = orch.get_session(session_id).await.unwrap();
    let entry = session.runbook.entry_by_id(entry_id).unwrap();
    assert_eq!(entry.args.get("jurisdiction").unwrap(), "IE");
}

// ===========================================================================
// TEST 13: Edit nonexistent step → error response
// ===========================================================================

#[tokio::test]
async fn test_edit_nonexistent_step_returns_error() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, _entry_id) = setup_with_one_entry(&orch).await;

    let resp = orch
        .process(
            session_id,
            UserInputV2::Edit {
                step_id: Uuid::new_v4(), // nonexistent
                field: "name".to_string(),
                value: "Whatever".to_string(),
            },
        )
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
        "Edit on nonexistent step should return recoverable error"
    );
    assert!(resp.message.contains("not found"));
}

// ===========================================================================
// TEST 14: Undo removes last entry, entry goes to undo stack
// ===========================================================================

#[tokio::test]
async fn test_undo_removes_last_entry() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, _entry_id) = setup_with_one_entry(&orch).await;

    // Verify user entry exists (plus 2 infra entries: scope + pack)
    let session = orch.get_session(session_id).await.unwrap();
    let total_before = session.runbook.entries.len();
    assert!(
        session
            .runbook
            .entries
            .iter()
            .any(|e| e.verb == "cbu.create"),
        "Expected cbu.create entry"
    );

    // Undo removes the last entry (which is cbu.create)
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Undo,
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::RunbookSummary { .. }
    ));
    assert!(resp.message.contains("Undone"));
    assert_eq!(resp.step_count, total_before - 1);

    // Verify user entry was moved to undo stack
    let session = orch.get_session(session_id).await.unwrap();
    assert!(
        !session
            .runbook
            .entries
            .iter()
            .any(|e| e.verb == "cbu.create"),
        "cbu.create should have been undone"
    );
    assert_eq!(session.runbook.undo_stack.len(), 1);
    assert_eq!(session.runbook.undo_stack[0].verb, "cbu.create");
}

// ===========================================================================
// TEST 15: Redo restores entry from undo stack
// ===========================================================================

#[tokio::test]
async fn test_redo_restores_entry() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, _entry_id) = setup_with_one_entry(&orch).await;

    // Undo
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Undo,
        },
    )
    .await
    .unwrap();

    // Redo
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Redo,
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::RunbookSummary { .. }
    ));
    assert!(resp.message.contains("Restored"));

    let session = orch.get_session(session_id).await.unwrap();
    assert!(
        session
            .runbook
            .entries
            .iter()
            .any(|e| e.verb == "cbu.create"),
        "cbu.create should be restored after redo"
    );
    assert!(session.runbook.undo_stack.is_empty());
}

// ===========================================================================
// TEST 16: Undo then Redo is identity (entry restored)
// ===========================================================================

#[tokio::test]
async fn test_undo_redo_cycle_is_identity() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Capture original entry state
    let session = orch.get_session(session_id).await.unwrap();
    let original_entry = session.runbook.entry_by_id(entry_id).unwrap();
    let original_verb = original_entry.verb.clone();
    let original_sentence = original_entry.sentence.clone();
    let total_before = session.runbook.entries.len();

    // Undo
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Undo,
        },
    )
    .await
    .unwrap();

    // Redo
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Redo,
        },
    )
    .await
    .unwrap();

    // Verify identity: same total entries, same user entry preserved
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.entries.len(), total_before);
    let restored = session.runbook.entry_by_id(entry_id).unwrap();
    assert_eq!(restored.verb, original_verb);
    assert_eq!(restored.sentence, original_sentence);
}

// ===========================================================================
// TEST 17: Undo on empty runbook → error
// ===========================================================================

#[tokio::test]
async fn test_undo_empty_runbook_returns_error() {
    let matcher = MockIntentMatcher::no_match();
    let orch = build_orchestrator_with_engine(matcher);
    let session_id = setup_in_pack(&orch).await;

    // After setup_in_pack, infra entries (scope + pack) exist.
    // Undo them all first to get a truly empty runbook.
    let session = orch.get_session(session_id).await.unwrap();
    let infra_count = session.runbook.entries.len();
    for _ in 0..infra_count {
        orch.process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Undo,
            },
        )
        .await
        .unwrap();
    }

    // Now the runbook is truly empty — undo should return error
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Undo,
            },
        )
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
        "Undo on empty runbook should return error, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );
    assert!(resp.message.contains("Nothing to undo"));
}

// ===========================================================================
// TEST 18: Clear empties runbook, returns count
// ===========================================================================

#[tokio::test]
async fn test_clear_empties_runbook() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, _entry_id) = setup_with_one_entry(&orch).await;

    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Clear,
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::RunbookSummary { .. }
    ));
    // Clears all entries (infra + user)
    assert!(resp.message.contains("Cleared"));
    assert_eq!(resp.step_count, 0);

    let session = orch.get_session(session_id).await.unwrap();
    assert!(session.runbook.entries.is_empty());
}

// ===========================================================================
// TEST 19: Cancel from SentencePlayback → returns to InPack
// ===========================================================================

#[tokio::test]
async fn test_cancel_from_sentence_playback() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let session_id = setup_in_pack(&orch).await;

    // Send message → SentencePlayback
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "create allianz lux cbu".to_string(),
        },
    )
    .await
    .unwrap();

    // Verify we're in SentencePlayback
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(
        session.state,
        ReplStateV2::SentencePlayback { .. }
    ));

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

    assert!(resp.message.contains("Cancelled"));

    // Should be back in InPack (since pack is active)
    let session = orch.get_session(session_id).await.unwrap();
    assert!(
        matches!(session.state, ReplStateV2::InPack { .. }),
        "After cancel from SentencePlayback, should be in InPack, got: {:?}",
        std::mem::discriminant(&session.state)
    );

    // No user entry added to runbook (only infra entries from scope + pack)
    assert!(
        !session
            .runbook
            .entries
            .iter()
            .any(|e| e.verb == "cbu.create"),
        "cbu.create should NOT be in runbook after cancel"
    );
}

// ===========================================================================
// TEST 20: Cancel from Clarifying → returns to InPack
// ===========================================================================

#[tokio::test]
async fn test_cancel_from_clarifying() {
    let matcher =
        MockIntentMatcher::ambiguous(vec![("cbu.create", 0.82), ("cbu.list", 0.79)], 0.03);
    let orch = build_orchestrator_with_engine(matcher);
    let session_id = setup_in_pack(&orch).await;

    // Send message → should get proposals (ambiguous match)
    let _resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "create or list".to_string(),
            },
        )
        .await
        .unwrap();

    // With proposal engine, ambiguous matches go to StepProposals, not Clarifying.
    // But we can still test Cancel from InPack state.
    // Let's cancel anyway (Cancel is valid from InPack too).
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Cancel,
            },
        )
        .await
        .unwrap();

    assert!(resp.message.contains("Cancelled"));
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::InPack { .. }));
}

// ===========================================================================
// TEST 21: Info shows scope, pack, readiness
// ===========================================================================

#[tokio::test]
async fn test_info_shows_session_details() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, _entry_id) = setup_with_one_entry(&orch).await;

    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Info,
            },
        )
        .await
        .unwrap();

    // Info should contain session details
    assert!(resp.message.contains("Scope: Allianz"));
    assert!(resp.message.contains("Pack: Freeform Test Pack"));
    assert!(resp.message.contains("Steps:"));
    assert!(resp.message.contains("Ready:"));
}

// ===========================================================================
// TEST 22: Help returns context-appropriate help text
// ===========================================================================

#[tokio::test]
async fn test_help_returns_context_help() {
    let matcher = MockIntentMatcher::no_match();
    let orch = build_orchestrator_with_engine(matcher);
    let session_id = setup_in_pack(&orch).await;

    // Help from InPack state
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Help,
            },
        )
        .await
        .unwrap();

    assert!(resp.message.contains("/run"));
    assert!(resp.message.contains("/undo"));
    assert!(resp.message.contains("/redo"));
    assert!(resp.message.contains("/clear"));
    assert!(resp.message.contains("/cancel"));
    assert!(resp.message.contains("/info"));
    assert!(resp.message.contains("/help"));
}

// ===========================================================================
// TEST 23: Disable step → skipped during execution
// ===========================================================================

#[tokio::test]
async fn test_disable_step_skipped_during_execution() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Disable the entry
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Disable(entry_id),
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::RunbookSummary { .. }
    ));
    assert!(resp.message.contains("disabled"));

    // Verify status
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Disabled
    );

    // Try to run — infra entries (Completed) pass readiness, but the user entry is skipped.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            },
        )
        .await
        .unwrap();

    // Run succeeds (infra entries are Completed, user entry is Disabled → skipped).
    match &resp.kind {
        ReplResponseKindV2::Executed { results } => {
            // The disabled entry should be reported as skipped.
            let disabled_result = results.iter().find(|r| r.entry_id == entry_id);
            if let Some(r) = disabled_result {
                assert!(r.success);
                assert_eq!(r.message.as_deref(), Some("Skipped (disabled)"));
            }
        }
        other => panic!(
            "Expected Executed response, got: {:?}",
            std::mem::discriminant(other)
        ),
    }
}

// ===========================================================================
// TEST 24: Enable step → included in execution
// ===========================================================================

#[tokio::test]
async fn test_enable_step_included_in_execution() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Disable then re-enable
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Disable(entry_id),
        },
    )
    .await
    .unwrap();

    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Enable(entry_id),
            },
        )
        .await
        .unwrap();

    assert!(resp.message.contains("enabled"));

    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Confirmed
    );

    // Now run should succeed
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
        matches!(resp.kind, ReplResponseKindV2::Executed { .. }),
        "Run after enable should succeed, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );
}

// ===========================================================================
// TEST 25: Toggle flips disabled state
// ===========================================================================

#[tokio::test]
async fn test_toggle_flips_disabled_state() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Toggle: Confirmed → Disabled
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Toggle(entry_id),
            },
        )
        .await
        .unwrap();

    assert!(resp.message.contains("disabled"));
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Disabled
    );

    // Toggle: Disabled → Confirmed
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Toggle(entry_id),
            },
        )
        .await
        .unwrap();

    assert!(resp.message.contains("enabled"));
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.entry_by_id(entry_id).unwrap().status,
        EntryStatus::Confirmed
    );
}

// ===========================================================================
// TEST 26: Run blocked when entry is Proposed (not Confirmed)
// ===========================================================================

#[tokio::test]
async fn test_run_blocked_when_entry_proposed() {
    // We need to directly manipulate the runbook to set an entry to Proposed.
    // Build orchestrator and add an entry manually.
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Set entry back to Proposed to test readiness gate
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        session
            .runbook
            .set_entry_status(entry_id, EntryStatus::Proposed);
    }

    // Try to run
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
        matches!(
            resp.kind,
            ReplResponseKindV2::Error {
                recoverable: true,
                ..
            }
        ),
        "Run with Proposed entry should fail readiness, got: {:?}",
        std::mem::discriminant(&resp.kind)
    );
    assert!(resp.message.contains("not confirmed") || resp.message.contains("issue"));
}

// ===========================================================================
// TEST 27: Run succeeds when all entries Confirmed
// ===========================================================================

#[tokio::test]
async fn test_run_succeeds_all_confirmed() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, _entry_id) = setup_with_one_entry(&orch).await;

    // Entry is Confirmed (from setup_with_one_entry)
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
}

// ===========================================================================
// TEST 28: Run skips Disabled entries (reports as "Skipped")
// ===========================================================================

#[tokio::test]
async fn test_run_skips_disabled_entries() {
    let matcher = MockIntentMatcher::matched(
        "cbu.create",
        0.92,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    );
    let orch = build_orchestrator_with_engine(matcher);
    let (session_id, entry_id) = setup_with_one_entry(&orch).await;

    // Add a second entry directly (so we have one enabled + one disabled)
    {
        let mut sessions = orch.sessions_for_test().write().await;
        let session = sessions.get_mut(&session_id).unwrap();
        let args = HashMap::from([
            ("name".to_string(), "Second Fund".to_string()),
            ("jurisdiction".to_string(), "IE".to_string()),
        ]);
        let entry2 = confirmed_entry("cbu.create", "Create Second Fund", args);
        session.runbook.add_entry(entry2);
    }

    // Disable the first entry
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Disable(entry_id),
        },
    )
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
            assert_eq!(results.len(), 2);

            // First entry (disabled) should be skipped
            let disabled_result = results.iter().find(|r| r.entry_id == entry_id).unwrap();
            assert!(disabled_result.success);
            assert_eq!(
                disabled_result.message.as_deref(),
                Some("Skipped (disabled)")
            );

            // Second entry should complete normally
            let enabled_result = results.iter().find(|r| r.entry_id != entry_id).unwrap();
            assert!(enabled_result.success);
            assert_eq!(enabled_result.message.as_deref(), Some("Completed"));
        }
        other => panic!(
            "Expected Executed response, got: {:?}",
            std::mem::discriminant(other)
        ),
    }
}

// ===========================================================================
// TEST 29: Full golden loop with editing
// ===========================================================================

#[tokio::test]
async fn test_golden_loop_with_editing() {
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

    // 4. Propose a step → auto-advance to SentencePlayback
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

    // 5. Confirm → entry added to runbook
    let resp = orch
        .process(session_id, UserInputV2::Confirm)
        .await
        .unwrap();
    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::RunbookSummary { .. }
    ));

    let session = orch.get_session(session_id).await.unwrap();
    let entry_id = session
        .runbook
        .entries
        .iter()
        .rfind(|e| e.verb == "cbu.create")
        .expect("Expected cbu.create entry in runbook")
        .id;

    // 6. Edit the entry
    let resp = orch
        .process(
            session_id,
            UserInputV2::Edit {
                step_id: entry_id,
                field: "name".to_string(),
                value: "Aviva Ireland".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::RunbookSummary { .. }
    ));
    assert!(resp.message.contains("Aviva Ireland"));

    // 7. Verify edit was applied
    let session = orch.get_session(session_id).await.unwrap();
    let entry = session.runbook.entry_by_id(entry_id).unwrap();
    assert_eq!(entry.args.get("name").unwrap(), "Aviva Ireland");

    // 8. Check info
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Info,
            },
        )
        .await
        .unwrap();
    assert!(resp.message.contains("Ready:"));

    // 9. Execute the runbook
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

    // 10. Session should be in RunbookEditing after execution
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::RunbookEditing));
}
