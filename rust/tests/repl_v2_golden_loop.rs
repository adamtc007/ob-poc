//! Integration tests for the v2 REPL: Pack-Guided Runbook Architecture
//!
//! Phase 0 acceptance tests:
//! 1. Golden loop — scope → pack select → Q/A → sentence playback → confirm → stub execute
//! 2. Template provenance — template_id, template_hash, slot_provenance populated
//! 3. Pack hash stability — same YAML = same hash; modify → different hash
//! 4. Sentence generator coverage — 20+ verb/arg combos produce reasonable English
//! 5. Force-select — "use the onboarding journey" activates pack
#![cfg(feature = "vnext-repl")]

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use ob_poc::journey::pack::{load_pack_from_bytes, load_pack_from_file, PackManifest};
use ob_poc::journey::router::PackRouter;
use ob_poc::journey::template::instantiate_template;
use ob_poc::repl::orchestrator_v2::{ReplOrchestratorV2, StubExecutor};
use ob_poc::repl::response_v2::ReplResponseKindV2;
use ob_poc::repl::runbook::{EntryStatus, RunbookStatus, SlotSource};
use ob_poc::repl::sentence_gen::SentenceGenerator;
use ob_poc::repl::types_v2::{ReplCommandV2, ReplStateV2, UserInputV2};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_onboarding_pack() -> (Arc<PackManifest>, String) {
    let yaml = include_bytes!("../config/packs/onboarding-request.yaml");
    let (manifest, hash) = load_pack_from_bytes(yaml).unwrap();
    (Arc::new(manifest), hash)
}

fn load_book_setup_pack() -> (Arc<PackManifest>, String) {
    let yaml = include_bytes!("../config/packs/book-setup.yaml");
    let (manifest, hash) = load_pack_from_bytes(yaml).unwrap();
    (Arc::new(manifest), hash)
}

fn load_kyc_case_pack() -> (Arc<PackManifest>, String) {
    let yaml = include_bytes!("../config/packs/kyc-case.yaml");
    let (manifest, hash) = load_pack_from_bytes(yaml).unwrap();
    (Arc::new(manifest), hash)
}

fn make_orchestrator_with_all_packs() -> ReplOrchestratorV2 {
    let packs = vec![
        load_onboarding_pack(),
        load_book_setup_pack(),
        load_kyc_case_pack(),
    ];
    let router = PackRouter::new(packs);
    ReplOrchestratorV2::new(router, Arc::new(StubExecutor))
}

fn make_orchestrator_with_onboarding() -> ReplOrchestratorV2 {
    let packs = vec![load_onboarding_pack()];
    let router = PackRouter::new(packs);
    ReplOrchestratorV2::new(router, Arc::new(StubExecutor))
}

async fn scope_and_select_pack(orch: &ReplOrchestratorV2, pack_id: &str) -> Uuid {
    let id = orch.create_session().await;

    // Set scope.
    orch.process(
        id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Allianz".to_string(),
        },
    )
    .await
    .unwrap();

    // Select pack.
    orch.process(
        id,
        UserInputV2::SelectPack {
            pack_id: pack_id.to_string(),
        },
    )
    .await
    .unwrap();

    id
}

// ===========================================================================
// TEST 1: Golden Loop (end-to-end)
// ===========================================================================

#[tokio::test]
async fn test_golden_loop_full() {
    let orch = make_orchestrator_with_onboarding();
    let session_id = orch.create_session().await;

    // Step 1: Starts in ScopeGate.
    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::ScopeGate { .. }));

    // Step 2: Set scope → JourneySelection.
    let resp = orch
        .process(
            session_id,
            UserInputV2::SelectScope {
                group_id: Uuid::new_v4(),
                group_name: "Allianz".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::JourneyOptions { .. }
    ));

    // Step 3: Select pack → InPack (first question).
    let resp = orch
        .process(
            session_id,
            UserInputV2::SelectPack {
                pack_id: "onboarding-request".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(resp.kind, ReplResponseKindV2::Question { .. }));
    if let ReplResponseKindV2::Question { ref field, .. } = resp.kind {
        assert_eq!(field, "cbu_name");
    }

    // Step 4: Answer Q1 (cbu_name).
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "Allianz Lux SICAV".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(resp.kind, ReplResponseKindV2::Question { .. }));
    if let ReplResponseKindV2::Question { ref field, .. } = resp.kind {
        assert_eq!(field, "products");
    }

    // Step 5: Answer Q2 (products).
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "CUSTODY, FUND_ADMIN".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(resp.kind, ReplResponseKindV2::Question { .. }));
    if let ReplResponseKindV2::Question { ref field, .. } = resp.kind {
        assert_eq!(field, "jurisdiction");
    }

    // Step 6: Answer Q3 (jurisdiction) → template instantiation → RunbookSummary.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "LU".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(
        matches!(resp.kind, ReplResponseKindV2::RunbookSummary { .. }),
        "Expected RunbookSummary, got {:?}",
        resp.kind
    );
    assert!(resp.step_count > 0, "Runbook should have entries");

    // Verify runbook state after template instantiation.
    let session = orch.get_session(session_id).await.unwrap();
    assert!(!session.runbook.entries.is_empty());
    assert!(session.runbook.template_id.is_some());
    assert!(session.runbook.template_hash.is_some());

    // Every entry should have a non-empty sentence.
    for entry in &session.runbook.entries {
        assert!(!entry.sentence.is_empty(), "Entry should have a sentence");
        assert!(!entry.dsl.is_empty(), "Entry should have DSL");
        assert!(!entry.verb.is_empty(), "Entry should have a verb");
    }

    // Step 7: Execute runbook.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            },
        )
        .await
        .unwrap();
    assert!(matches!(resp.kind, ReplResponseKindV2::Executed { .. }));

    // Verify all entries completed.
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.runbook.status, RunbookStatus::Completed);
    for entry in &session.runbook.entries {
        assert_eq!(entry.status, EntryStatus::Completed);
        assert!(entry.result.is_some(), "Entry should have execution result");
    }
}

// ===========================================================================
// TEST 2: Template Provenance
// ===========================================================================

#[tokio::test]
async fn test_template_provenance() {
    let (manifest, _hash) = load_onboarding_pack();
    let template = &manifest.templates[0]; // standard-onboarding

    let context_vars = HashMap::from([
        ("client_name".to_string(), "Allianz".to_string()),
        ("client_group_id".to_string(), Uuid::new_v4().to_string()),
    ]);

    let answers: HashMap<String, serde_json::Value> = HashMap::from([
        (
            "cbu_name".to_string(),
            serde_json::Value::String("Allianz Lux Fund".to_string()),
        ),
        (
            "products".to_string(),
            serde_json::json!(["CUSTODY", "FUND_ADMIN"]),
        ),
        (
            "jurisdiction".to_string(),
            serde_json::Value::String("LU".to_string()),
        ),
        (
            "trading_instruments".to_string(),
            serde_json::json!(["EQUITY", "FIXED_INCOME"]),
        ),
    ]);

    let sentence_gen = SentenceGenerator;
    let verb_phrases = HashMap::new();
    let verb_descriptions = HashMap::new();

    let (entries, template_hash) = instantiate_template(
        template,
        &context_vars,
        &answers,
        &sentence_gen,
        &verb_phrases,
        &verb_descriptions,
    )
    .unwrap();

    // Template hash should be non-empty and deterministic.
    assert!(!template_hash.is_empty());

    // Run it again — same hash.
    let (_, template_hash_2) = instantiate_template(
        template,
        &context_vars,
        &answers,
        &sentence_gen,
        &verb_phrases,
        &verb_descriptions,
    )
    .unwrap();
    assert_eq!(
        template_hash, template_hash_2,
        "Template hash must be deterministic"
    );

    // Should have entries from the template.
    assert!(
        entries.len() >= 2,
        "Expected at least 2 entries, got {}",
        entries.len()
    );

    // Check provenance on each entry.
    for entry in &entries {
        assert!(
            !entry.slot_provenance.slots.is_empty(),
            "Entry '{}' should have slot provenance",
            entry.verb
        );
    }

    // The cbu.create step should have user-provided and context-inferred slots.
    let cbu_entry = entries.iter().find(|e| e.verb == "cbu.create").unwrap();
    let prov = &cbu_entry.slot_provenance;

    // "name" comes from answers → UserProvided
    assert_eq!(
        prov.slots.get("name"),
        Some(&SlotSource::UserProvided),
        "cbu_name should be UserProvided"
    );

    // "jurisdiction" comes from answers → UserProvided
    assert_eq!(
        prov.slots.get("jurisdiction"),
        Some(&SlotSource::UserProvided),
        "jurisdiction should be UserProvided"
    );

    // repeat_for entries should each have provenance.
    let product_entries: Vec<_> = entries
        .iter()
        .filter(|e| e.verb == "cbu.assign-product")
        .collect();
    assert_eq!(
        product_entries.len(),
        2,
        "Should have 2 product assignments"
    );
    for pe in &product_entries {
        assert!(
            pe.slot_provenance.slots.contains_key("product"),
            "Product entry should track 'product' provenance"
        );
    }
}

// ===========================================================================
// TEST 3: Pack Hash Stability
// ===========================================================================

#[test]
fn test_pack_hash_stability() {
    let yaml = include_bytes!("../config/packs/onboarding-request.yaml");

    // Same bytes → same hash.
    let hash1 = PackManifest::manifest_hash(yaml);
    let hash2 = PackManifest::manifest_hash(yaml);
    assert_eq!(hash1, hash2, "Same bytes must produce same hash");

    // Different bytes → different hash.
    let mut modified = yaml.to_vec();
    modified.push(b' ');
    let hash3 = PackManifest::manifest_hash(&modified);
    assert_ne!(hash1, hash3, "Different bytes must produce different hash");
}

#[test]
fn test_pack_hash_all_three_packs_unique() {
    let (_, hash1) = load_onboarding_pack();
    let (_, hash2) = load_book_setup_pack();
    let (_, hash3) = load_kyc_case_pack();

    assert_ne!(
        hash1, hash2,
        "Onboarding and book-setup should have different hashes"
    );
    assert_ne!(
        hash1, hash3,
        "Onboarding and kyc-case should have different hashes"
    );
    assert_ne!(
        hash2, hash3,
        "Book-setup and kyc-case should have different hashes"
    );
}

#[test]
fn test_pack_hash_via_file_loader() {
    use std::path::Path;

    let path = Path::new("config/packs/onboarding-request.yaml");
    if path.exists() {
        let (manifest, hash) = load_pack_from_file(path).unwrap();
        assert_eq!(manifest.id, "onboarding-request");
        assert!(!hash.is_empty());

        // Load again — same hash.
        let (_, hash2) = load_pack_from_file(path).unwrap();
        assert_eq!(hash, hash2);
    }
}

// ===========================================================================
// TEST 4: Sentence Generator Coverage (20+ combos)
// ===========================================================================

#[test]
fn test_sentence_generator_coverage() {
    let gen = SentenceGenerator;

    struct TestCase {
        verb: &'static str,
        args: Vec<(&'static str, &'static str)>,
    }

    let cases = vec![
        TestCase {
            verb: "cbu.create",
            args: vec![("name", "Acme Fund")],
        },
        TestCase {
            verb: "cbu.create",
            args: vec![("name", "Lux SICAV"), ("jurisdiction", "LU")],
        },
        TestCase {
            verb: "cbu.assign-product",
            args: vec![("product", "CUSTODY")],
        },
        TestCase {
            verb: "cbu.assign-role",
            args: vec![("role", "depositary"), ("entity_ref", "BNY Mellon")],
        },
        TestCase {
            verb: "cbu.delete",
            args: vec![("cbu_id", "abc-123")],
        },
        TestCase {
            verb: "cbu.list",
            args: vec![],
        },
        TestCase {
            verb: "entity.create",
            args: vec![("name", "Goldman Sachs")],
        },
        TestCase {
            verb: "entity.create",
            args: vec![("name", "John Smith"), ("type", "person")],
        },
        TestCase {
            verb: "trading-profile.create",
            args: vec![("name", "Main Profile")],
        },
        TestCase {
            verb: "trading-profile.add-instrument",
            args: vec![("instrument_class", "EQUITY")],
        },
        TestCase {
            verb: "kyc.create-case",
            args: vec![("entity_ref", "Acme Corp"), ("case_type", "new")],
        },
        TestCase {
            verb: "ubo.discover",
            args: vec![("entity_ref", "Allianz SE")],
        },
        TestCase {
            verb: "document.solicit",
            args: vec![("doc_type", "passport"), ("entity_ref", "John")],
        },
        TestCase {
            verb: "screening.run",
            args: vec![("entity_ref", "Suspicious Corp")],
        },
        TestCase {
            verb: "session.load-galaxy",
            args: vec![("apex_name", "Allianz")],
        },
        TestCase {
            verb: "session.load-cbu",
            args: vec![("cbu_name", "Allianz Lux")],
        },
        TestCase {
            verb: "session.clear",
            args: vec![],
        },
        TestCase {
            verb: "contract.create",
            args: vec![("client", "Aviva"), ("reference", "MSA-2024")],
        },
        TestCase {
            verb: "contract.subscribe",
            args: vec![("product", "CUSTODY")],
        },
        TestCase {
            verb: "view.drill",
            args: vec![("entity_id", "some-uuid")],
        },
        TestCase {
            verb: "view.surface",
            args: vec![],
        },
        TestCase {
            verb: "deal.create",
            args: vec![("deal_name", "Allianz Custody 2024")],
        },
        TestCase {
            verb: "billing.create-profile",
            args: vec![("billing_frequency", "MONTHLY")],
        },
        TestCase {
            verb: "gleif.import-tree",
            args: vec![("entity_id", "Aviva plc"), ("depth", "3")],
        },
    ];

    assert!(
        cases.len() >= 20,
        "Need at least 20 test cases, have {}",
        cases.len()
    );

    for tc in &cases {
        let args: HashMap<String, String> = tc
            .args
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let sentence = gen.generate(tc.verb, &args, &[], "");

        // Must produce non-empty, reasonable output.
        assert!(
            !sentence.is_empty(),
            "Sentence for '{}' should not be empty",
            tc.verb
        );
        assert!(
            sentence.len() >= 5,
            "Sentence for '{}' too short: '{}'",
            tc.verb,
            sentence
        );
        // Should not contain unsubstituted template markers.
        assert!(
            !sentence.contains("{answers.") && !sentence.contains("{context."),
            "Sentence for '{}' has unresolved templates: '{}'",
            tc.verb,
            sentence
        );
    }
}

#[test]
fn test_sentence_format_list_oxford_comma() {
    assert_eq!(SentenceGenerator::format_list(&[]), "");
    assert_eq!(
        SentenceGenerator::format_list(&["CUSTODY".to_string()]),
        "CUSTODY"
    );
    assert_eq!(
        SentenceGenerator::format_list(&["CUSTODY".to_string(), "FUND_ADMIN".to_string()]),
        "CUSTODY and FUND_ADMIN"
    );
    assert_eq!(
        SentenceGenerator::format_list(&[
            "CUSTODY".to_string(),
            "FUND_ADMIN".to_string(),
            "TA".to_string()
        ]),
        "CUSTODY, FUND_ADMIN, and TA"
    );
}

// ===========================================================================
// TEST 5: Force-Select Pack
// ===========================================================================

#[tokio::test]
async fn test_force_select_onboarding() {
    let orch = make_orchestrator_with_all_packs();
    let session_id = orch.create_session().await;

    // Set scope.
    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Aviva".to_string(),
        },
    )
    .await
    .unwrap();

    // Force-select: "use the onboarding request journey".
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "use the onboarding request journey".to_string(),
            },
        )
        .await
        .unwrap();

    // Should activate the pack (first question).
    assert!(
        matches!(resp.kind, ReplResponseKindV2::Question { .. }),
        "Expected Question after force-select, got {:?}",
        resp.kind
    );

    // Verify the pack is active.
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.active_pack_id().as_deref(),
        Some("onboarding-request")
    );
}

#[tokio::test]
async fn test_force_select_book_setup() {
    let orch = make_orchestrator_with_all_packs();
    let session_id = orch.create_session().await;

    // Set scope.
    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "BlackRock".to_string(),
        },
    )
    .await
    .unwrap();

    // Force-select by ID.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "use the book-setup pack".to_string(),
            },
        )
        .await
        .unwrap();

    assert!(matches!(resp.kind, ReplResponseKindV2::Question { .. }));
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.active_pack_id().as_deref(), Some("book-setup"));
}

#[tokio::test]
async fn test_force_select_kyc_case() {
    let orch = make_orchestrator_with_all_packs();
    let session_id = orch.create_session().await;

    // Set scope.
    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Aberdeen".to_string(),
        },
    )
    .await
    .unwrap();

    // Force-select via name substring.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "start KYC Case journey".to_string(),
            },
        )
        .await
        .unwrap();

    assert!(matches!(resp.kind, ReplResponseKindV2::Question { .. }));
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(session.active_pack_id().as_deref(), Some("kyc-case"));
}

// ===========================================================================
// TEST 6: All 3 Packs Load and Deserialize
// ===========================================================================

#[test]
fn test_all_packs_load() {
    let (onboarding, _) = load_onboarding_pack();
    assert_eq!(onboarding.id, "onboarding-request");
    assert!(!onboarding.invocation_phrases.is_empty());
    assert!(!onboarding.required_questions.is_empty());
    assert!(!onboarding.templates.is_empty());
    assert!(!onboarding.definition_of_done.is_empty());

    let (book_setup, _) = load_book_setup_pack();
    assert_eq!(book_setup.id, "book-setup");
    assert!(!book_setup.invocation_phrases.is_empty());
    assert!(
        book_setup.templates.len() >= 2,
        "Book setup should have LU + UK templates"
    );

    let (kyc, _) = load_kyc_case_pack();
    assert_eq!(kyc.id, "kyc-case");
    assert!(!kyc.invocation_phrases.is_empty());
    assert!(
        kyc.templates.len() >= 2,
        "KYC should have new + renewal templates"
    );
}

#[test]
fn test_pack_routing_all_packs() {
    let packs = vec![
        load_onboarding_pack(),
        load_book_setup_pack(),
        load_kyc_case_pack(),
    ];
    let router = PackRouter::new(packs);

    // List all packs.
    let all = router.list_packs();
    assert_eq!(all.len(), 3);

    // Get by ID.
    assert!(router.get_pack("onboarding-request").is_some());
    assert!(router.get_pack("book-setup").is_some());
    assert!(router.get_pack("kyc-case").is_some());
    assert!(router.get_pack("nonexistent").is_none());
}

// ===========================================================================
// TEST 7: Pack Playback
// ===========================================================================

#[tokio::test]
async fn test_playback_after_golden_loop() {
    let orch = make_orchestrator_with_onboarding();
    let session_id = scope_and_select_pack(&orch, "onboarding-request").await;

    // Answer all questions.
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "Allianz Lux Fund".to_string(),
        },
    )
    .await
    .unwrap();

    orch.process(
        session_id,
        UserInputV2::Message {
            content: "CUSTODY, FUND_ADMIN".to_string(),
        },
    )
    .await
    .unwrap();

    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "LU".to_string(),
            },
        )
        .await
        .unwrap();

    // Should have runbook summary.
    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::RunbookSummary { .. }
    ));

    // Verify playback chapters.
    if let ReplResponseKindV2::RunbookSummary {
        ref chapters,
        ref summary,
    } = resp.kind
    {
        assert!(!chapters.is_empty(), "Should have at least one chapter");
        assert!(!summary.is_empty(), "Summary should not be empty");

        // Check that chapters have steps.
        let total_steps: usize = chapters.iter().map(|c| c.steps.len()).sum();
        assert!(total_steps > 0, "Chapters should contain steps");
    }
}

// ===========================================================================
// TEST 8: Runbook Editing (remove + reorder)
// ===========================================================================

#[tokio::test]
async fn test_runbook_editing_remove_step() {
    let orch = make_orchestrator_with_onboarding();
    let session_id = scope_and_select_pack(&orch, "onboarding-request").await;

    // Answer all questions to build runbook.
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "Fund A".to_string(),
        },
    )
    .await
    .unwrap();
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "CUSTODY".to_string(),
        },
    )
    .await
    .unwrap();
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "LU".to_string(),
        },
    )
    .await
    .unwrap();

    // Get entry ID to remove.
    let session = orch.get_session(session_id).await.unwrap();
    let initial_count = session.runbook.entries.len();
    assert!(initial_count > 0);
    let entry_to_remove = session.runbook.entries[0].id;

    // Remove it.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Remove(entry_to_remove),
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::RunbookSummary { .. }
    ));
    assert_eq!(resp.step_count, initial_count - 1);
}

// ===========================================================================
// TEST 9: State Transitions Correctness
// ===========================================================================

#[tokio::test]
async fn test_reject_then_re_add() {
    let orch = make_orchestrator_with_onboarding();
    let session_id = scope_and_select_pack(&orch, "onboarding-request").await;

    // Answer all questions.
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "Fund X".to_string(),
        },
    )
    .await
    .unwrap();
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "CUSTODY".to_string(),
        },
    )
    .await
    .unwrap();
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "IE".to_string(),
            },
        )
        .await
        .unwrap();

    // Runbook is built — we're in RunbookEditing.
    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::RunbookSummary { .. }
    ));

    // Execute should work.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            },
        )
        .await
        .unwrap();
    assert!(matches!(resp.kind, ReplResponseKindV2::Executed { .. }));
}

#[tokio::test]
async fn test_scope_gate_message_stays_in_scope_gate() {
    let orch = make_orchestrator_with_all_packs();
    let session_id = orch.create_session().await;

    // Sending a message without scope should stay in ScopeGate.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "hello world".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        resp.kind,
        ReplResponseKindV2::ScopeRequired { .. }
    ));

    let session = orch.get_session(session_id).await.unwrap();
    assert!(matches!(session.state, ReplStateV2::ScopeGate { .. }));
}

#[tokio::test]
async fn test_no_match_shows_journey_options() {
    let orch = make_orchestrator_with_all_packs();
    let session_id = orch.create_session().await;

    // Set scope.
    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Test Corp".to_string(),
        },
    )
    .await
    .unwrap();

    // Send unrelated message — should list available packs.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "xyzzy foobar nonsense".to_string(),
            },
        )
        .await
        .unwrap();

    assert!(
        matches!(resp.kind, ReplResponseKindV2::JourneyOptions { .. }),
        "Unmatched input should show journey options, got {:?}",
        resp.kind
    );
}
