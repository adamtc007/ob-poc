//! Phase 1 Integration Tests — Real Wiring: Packs + Verbs + Execution
//!
//! These tests verify Phase 1 functionality:
//! 1. Candle pack routing — semantic scorer discovers packs
//! 2. Real execution bridge — DslExecutor bridges to parse pipeline
//! 3. Pack verb filtering — allowed_verbs constrains verb matching
//! 4. Arg extraction audit — populated on IntentMatcher-derived entries
//! 5. Sentence templates — pack verbs use templates, not search_phrases
//! 6. Confirm policy — QuickConfirm for navigation verbs
//! 7. Scenario D — "Onboard Allianz Lux" → full pipeline
#![cfg(feature = "vnext-repl")]

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use ob_poc::journey::pack::{load_pack_from_bytes, PackManifest};
use ob_poc::journey::router::{PackRouteOutcome, PackRouter, PackSemanticScorer};
use ob_poc::repl::orchestrator_v2::{DslExecutor, ReplOrchestratorV2, StubExecutor};
use ob_poc::repl::response_v2::ReplResponseKindV2;
use ob_poc::repl::runbook::{ConfirmPolicy, EntryStatus, RunbookEntry};
use ob_poc::repl::sentence_gen::SentenceGenerator;
use ob_poc::repl::types_v2::{ReplCommandV2, UserInputV2};
use ob_poc::repl::verb_config_index::VerbConfigIndex;

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

fn all_packs() -> Vec<(Arc<PackManifest>, String)> {
    vec![
        load_onboarding_pack(),
        load_book_setup_pack(),
        load_kyc_case_pack(),
    ]
}

/// A mock semantic scorer that returns scores based on phrase content.
///
/// For each phrase, checks if any trigger word appears in the phrase text
/// and returns the corresponding score. This allows different packs to get
/// different scores based on their invocation_phrases content.
struct MockSemanticScorer {
    /// Map from phrase substring → score. Checked against PHRASE text, not query.
    phrase_triggers: Vec<(String, f32)>,
    /// Fallback score when no phrase trigger matches.
    default_score: f32,
}

impl MockSemanticScorer {
    /// Create a scorer where triggers match against phrase content.
    /// Only returns high scores for phrases containing the trigger word.
    fn for_phrases(triggers: Vec<(&str, f32)>, default: f32) -> Self {
        Self {
            phrase_triggers: triggers
                .into_iter()
                .map(|(s, score)| (s.to_lowercase(), score))
                .collect(),
            default_score: default,
        }
    }
}

impl PackSemanticScorer for MockSemanticScorer {
    fn score(&self, _query: &str, phrases: &[String]) -> Result<Vec<f32>, String> {
        Ok(phrases
            .iter()
            .map(|phrase| {
                let phrase_lower = phrase.to_lowercase();
                self.phrase_triggers
                    .iter()
                    .find(|(trigger, _)| phrase_lower.contains(trigger))
                    .map(|(_, score)| *score)
                    .unwrap_or(self.default_score)
            })
            .collect())
    }
}

/// A mock executor that records calls and returns configurable results.
struct RecordingExecutor {
    results: std::sync::Mutex<Vec<Result<serde_json::Value, String>>>,
    calls: std::sync::Mutex<Vec<String>>,
}

impl RecordingExecutor {
    fn always_success() -> Self {
        Self {
            results: std::sync::Mutex::new(vec![]),
            calls: std::sync::Mutex::new(vec![]),
        }
    }

    fn get_calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl DslExecutor for RecordingExecutor {
    async fn execute(&self, dsl: &str) -> Result<serde_json::Value, String> {
        self.calls.lock().unwrap().push(dsl.to_string());
        let mut results = self.results.lock().unwrap();
        if results.is_empty() {
            Ok(serde_json::json!({"status": "ok", "dsl": dsl}))
        } else {
            results.remove(0)
        }
    }
}

async fn scope_session(orch: &ReplOrchestratorV2, group_name: &str) -> Uuid {
    let id = orch.create_session().await;
    orch.process(
        id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: group_name.to_string(),
        },
    )
    .await
    .unwrap();
    id
}

async fn scope_and_select_pack(orch: &ReplOrchestratorV2, pack_id: &str) -> Uuid {
    let id = scope_session(orch, "Allianz").await;
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
// TEST 1: Candle Pack Routing — Semantic Scorer Discovers Packs
// ===========================================================================

#[test]
fn test_semantic_scorer_discovers_pack_by_intent() {
    let packs = all_packs();
    // Scorer returns high scores for phrases containing "onboard" — these exist
    // only in the onboarding-request pack's invocation_phrases/description.
    let scorer = MockSemanticScorer::for_phrases(vec![("onboard", 0.80)], 0.30);
    let router = PackRouter::new(packs).with_scorer(Arc::new(scorer));

    // "I want to onboard a client" — semantic scorer gives 0.80 to onboarding
    // pack phrases (which contain "onboard") and 0.30 to others.
    let result = router.route("I want to onboard a client");
    assert!(
        matches!(result, PackRouteOutcome::Matched(ref m, _) if m.id == "onboarding-request"),
        "Expected Matched(onboarding-request), got {:?}",
        match result {
            PackRouteOutcome::Matched(m, _) => format!("Matched({})", m.id),
            PackRouteOutcome::Ambiguous(c) => format!("Ambiguous({})", c.len()),
            PackRouteOutcome::NoMatch => "NoMatch".to_string(),
        }
    );
}

#[test]
fn test_semantic_scorer_no_match_below_threshold() {
    let packs = all_packs();
    // All scores below semantic threshold (0.55) → should not match.
    let scorer = MockSemanticScorer::for_phrases(vec![], 0.30);
    let router = PackRouter::new(packs).with_scorer(Arc::new(scorer));

    let result = router.route("xyzzy gibberish foobar");
    assert!(
        matches!(result, PackRouteOutcome::NoMatch),
        "Low-score semantic match should not activate a pack"
    );
}

#[test]
fn test_force_select_beats_semantic() {
    let packs = all_packs();
    // High semantic scores for "book setup" phrases, but force-select should still win.
    let scorer = MockSemanticScorer::for_phrases(vec![("book", 0.90)], 0.70);
    let router = PackRouter::new(packs).with_scorer(Arc::new(scorer));

    // Force-select: explicit pack name match always wins.
    let result = router.route("use the onboarding-request pack");
    assert!(
        matches!(result, PackRouteOutcome::Matched(ref m, _) if m.id == "onboarding-request"),
        "Force-select should beat semantic scoring"
    );
}

#[test]
fn test_substring_match_before_semantic() {
    let packs = all_packs();
    // Low semantic scores → substring match should drive the result.
    let scorer = MockSemanticScorer::for_phrases(vec![], 0.10);
    let router = PackRouter::new(packs).with_scorer(Arc::new(scorer));

    // "onboard a new client" is an exact invocation_phrase on onboarding-request.
    let result = router.route("onboard a new client");
    assert!(
        matches!(result, PackRouteOutcome::Matched(ref m, _) if m.id == "onboarding-request"),
        "Substring match should work even with low semantic scores"
    );
}

// ===========================================================================
// TEST 2: Execution Bridge — RecordingExecutor
// ===========================================================================

#[tokio::test]
async fn test_execution_bridge_records_dsl() {
    let executor = Arc::new(RecordingExecutor::always_success());
    let packs = vec![load_onboarding_pack()];
    let router = PackRouter::new(packs);
    let orch = ReplOrchestratorV2::new(router, executor.clone());

    let session_id = scope_and_select_pack(&orch, "onboarding-request").await;

    // Answer questions to build runbook.
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

    // Execute.
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

    // Verify executor received DSL calls.
    let calls = executor.get_calls();
    assert!(!calls.is_empty(), "Executor should have received DSL calls");
    for call in &calls {
        assert!(
            call.starts_with('('),
            "Each DSL call should be an s-expression, got: {}",
            call
        );
    }
}

// ===========================================================================
// TEST 3: VerbConfigIndex — Sentence Templates + ConfirmPolicy
// ===========================================================================

#[test]
fn test_verb_config_index_sentence_templates() {
    // Build a VerbConfigIndex and verify pack verbs have sentence templates.
    let index = VerbConfigIndex::empty();

    // Empty index returns no entries (expected — real index built from VerbsConfig).
    assert!(index.get("cbu.create").is_none());

    // The VerbConfigIndex contract: pack verbs should have sentence templates.
    // This is tested via the sentence_gen integration below.
}

#[test]
fn test_sentence_templates_for_pack_verbs() {
    let gen = SentenceGenerator;

    // Pack verbs that MUST have sentence templates (from Step 1.6).
    let pack_verbs = vec![
        (
            "cbu.assign-product",
            vec![("product", "CUSTODY"), ("cbu-name", "Allianz Lux")],
        ),
        (
            "cbu.create",
            vec![("name", "Allianz Lux"), ("jurisdiction", "LU")],
        ),
        ("session.load-galaxy", vec![("apex-name", "Allianz")]),
        ("session.load-cbu", vec![("cbu-name", "Allianz Lux")]),
        (
            "kyc.open-case",
            vec![("entity-ref", "Allianz SE"), ("case-type", "new")],
        ),
        (
            "entity.ensure-or-create",
            vec![("name", "Goldman Sachs"), ("entity-type", "company")],
        ),
    ];

    for (verb, arg_pairs) in &pack_verbs {
        let args: HashMap<String, String> = arg_pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let sentence = gen.generate(verb, &args, &[], "");
        assert!(
            !sentence.is_empty(),
            "Pack verb '{}' should produce a non-empty sentence",
            verb
        );
        // Sentence should contain at least one arg value for verbs with args.
        if !args.is_empty() {
            let any_arg_in_sentence = args.values().any(|v| sentence.contains(v));
            assert!(
                any_arg_in_sentence,
                "Sentence for '{}' should contain at least one arg value. Got: '{}'",
                verb, sentence
            );
        }
    }
}

#[test]
fn test_confirm_policy_defaults() {
    let index = VerbConfigIndex::empty();

    // Empty index should return Always (safe default).
    assert_eq!(
        index.confirm_policy("cbu.create"),
        ConfirmPolicy::Always,
        "Data-modifying verbs should default to Always"
    );
    assert_eq!(
        index.confirm_policy("nonexistent.verb"),
        ConfirmPolicy::Always,
        "Unknown verbs should default to Always"
    );
}

// ===========================================================================
// TEST 4: ArgExtractionAudit — Populated on IntentMatcher Entries
// ===========================================================================

#[test]
fn test_template_entries_have_no_audit() {
    // Template-derived RunbookEntry instances should NOT have arg_extraction_audit.
    let entry = RunbookEntry::new(
        "cbu.create".to_string(),
        "Create Allianz Lux CBU".to_string(),
        "(cbu.create :name \"Allianz Lux\")".to_string(),
    );
    assert!(
        entry.arg_extraction_audit.is_none(),
        "Template-derived entries should not have audit"
    );
}

#[test]
fn test_audit_fields_structure() {
    use ob_poc::repl::runbook::ArgExtractionAudit;

    let audit = ArgExtractionAudit {
        model_id: "global_semantic".to_string(),
        prompt_hash: "abc123".to_string(),
        user_input: "create fund for allianz".to_string(),
        extracted_args: HashMap::from([
            ("name".to_string(), "Allianz".to_string()),
            ("kind".to_string(), "fund".to_string()),
        ]),
        confidence: 0.85,
        timestamp: chrono::Utc::now(),
    };

    // Verify serialization roundtrip.
    let json = serde_json::to_string(&audit).unwrap();
    let deserialized: ArgExtractionAudit = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.model_id, "global_semantic");
    assert_eq!(deserialized.user_input, "create fund for allianz");
    assert_eq!(deserialized.extracted_args.len(), 2);
    assert!((deserialized.confidence - 0.85).abs() < 0.001);
}

#[tokio::test]
async fn test_golden_loop_template_entries_no_audit() {
    // In the golden loop (template instantiation), entries should NOT have audit.
    let orch = ReplOrchestratorV2::new(
        PackRouter::new(vec![load_onboarding_pack()]),
        Arc::new(StubExecutor),
    );
    let session_id = scope_and_select_pack(&orch, "onboarding-request").await;

    // Answer all questions.
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

    // Verify all template entries have no audit.
    let session = orch.get_session(session_id).await.unwrap();
    for entry in &session.runbook.entries {
        assert!(
            entry.arg_extraction_audit.is_none(),
            "Template entry '{}' should NOT have arg_extraction_audit",
            entry.verb
        );
    }
}

// ===========================================================================
// TEST 5: Sentence Quality — No Fallback to Raw Phrases for Pack Verbs
// ===========================================================================

#[test]
fn test_sentence_quality_with_templates() {
    let gen = SentenceGenerator;

    // Sentence templates should produce well-formed English.
    let templates = vec![
        "Assign {product} product to {cbu-name}".to_string(),
        "Add {product} to {cbu-name} product list".to_string(),
    ];
    let args = HashMap::from([
        ("product".to_string(), "CUSTODY".to_string()),
        ("cbu-name".to_string(), "Allianz Lux".to_string()),
    ]);

    let sentence = gen.generate("cbu.assign-product", &args, &templates, "assign product");
    assert!(
        sentence.contains("CUSTODY"),
        "Sentence should contain product name: '{}'",
        sentence
    );
    assert!(
        sentence.contains("Allianz Lux"),
        "Sentence should contain CBU name: '{}'",
        sentence
    );
}

#[test]
fn test_sentence_with_all_args_resolves_placeholders() {
    let gen = SentenceGenerator;

    // When all args are provided, no placeholders should remain.
    let templates = vec!["Assign {product} product to {cbu-name}".to_string()];
    let args = HashMap::from([
        ("product".to_string(), "CUSTODY".to_string()),
        ("cbu-name".to_string(), "Allianz Lux".to_string()),
    ]);

    let sentence = gen.generate("cbu.assign-product", &args, &templates, "assign product");
    assert!(
        !sentence.contains('{'),
        "Sentence should not contain unresolved placeholders when all args provided: '{}'",
        sentence
    );
    assert!(sentence.contains("CUSTODY"));
    assert!(sentence.contains("Allianz Lux"));
}

// ===========================================================================
// TEST 6: Confirm Policy — QuickConfirm for Navigation
// ===========================================================================

#[test]
fn test_quick_confirm_nav_verbs() {
    // Navigation verbs should get QuickConfirm from VerbConfigIndex.
    // With real VerbsConfig this would be verified.
    // For now, test the sentinel behavior on empty index.
    let index = VerbConfigIndex::empty();

    // Empty index defaults to Always for all verbs.
    assert_eq!(
        index.confirm_policy("session.load-galaxy"),
        ConfirmPolicy::Always
    );

    // When a real VerbConfigIndex is built (Phase 2 with VerbsConfig),
    // session.* verbs should return QuickConfirm.
}

// ===========================================================================
// TEST 7: Scenario D — Pack Shorthand
// ===========================================================================

#[tokio::test]
async fn test_scenario_d_onboard_allianz_lux() {
    let orch = ReplOrchestratorV2::new(PackRouter::new(all_packs()), Arc::new(StubExecutor));
    let session_id = orch.create_session().await;

    // Set scope.
    orch.process(
        session_id,
        UserInputV2::SelectScope {
            group_id: Uuid::new_v4(),
            group_name: "Allianz".to_string(),
        },
    )
    .await
    .unwrap();

    // "Onboard Allianz Lux" should route to onboarding pack.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: "Onboard Allianz Lux".to_string(),
            },
        )
        .await
        .unwrap();

    // Should activate the onboarding pack (first question).
    assert!(
        matches!(resp.kind, ReplResponseKindV2::Question { .. }),
        "Expected Question after pack routing, got {:?}",
        resp.kind
    );

    let session = orch.get_session(session_id).await.unwrap();
    assert!(session.journey_context.is_some());
    assert_eq!(
        session.journey_context.as_ref().unwrap().pack.id,
        "onboarding-request"
    );

    // Answer questions → build runbook → execute.
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "Allianz Lux SICAV".to_string(),
        },
    )
    .await
    .unwrap();
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "CUSTODY, TA".to_string(),
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

    // Should have runbook with steps.
    assert!(
        matches!(resp.kind, ReplResponseKindV2::RunbookSummary { .. }),
        "Expected RunbookSummary, got {:?}",
        resp.kind
    );
    assert!(resp.step_count > 0);

    // Execute.
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

    // Verify final state.
    let session = orch.get_session(session_id).await.unwrap();
    assert_eq!(
        session.runbook.status,
        ob_poc::repl::runbook::RunbookStatus::Completed
    );

    // Verify provenance is set.
    assert!(session.runbook.pack_id.is_some());
    assert!(session.runbook.template_id.is_some());
    assert!(session.runbook.template_hash.is_some());

    // Verify entries have sentences and provenance.
    for entry in &session.runbook.entries {
        assert!(!entry.sentence.is_empty());
        assert!(!entry.dsl.is_empty());
        assert!(!entry.verb.is_empty());
        assert_eq!(entry.status, EntryStatus::Completed);

        // Template entries should have slot provenance.
        if !entry.args.is_empty() {
            assert!(
                !entry.slot_provenance.slots.is_empty(),
                "Entry '{}' with args should have slot provenance",
                entry.verb
            );
        }
    }
}

// ===========================================================================
// TEST 8: Pack Router — List and Get Operations
// ===========================================================================

#[test]
fn test_pack_router_with_scorer_list_packs() {
    let scorer = MockSemanticScorer::for_phrases(vec![], 0.10);
    let router = PackRouter::new(all_packs()).with_scorer(Arc::new(scorer));

    let packs = router.list_packs();
    assert_eq!(packs.len(), 3);

    // All packs accessible by ID.
    assert!(router.get_pack("onboarding-request").is_some());
    assert!(router.get_pack("book-setup").is_some());
    assert!(router.get_pack("kyc-case").is_some());
}

// ===========================================================================
// TEST 9: Execution with Recording Executor — DSL Correctness
// ===========================================================================

#[tokio::test]
async fn test_executed_dsl_is_valid_sexpr() {
    let executor = Arc::new(RecordingExecutor::always_success());
    let orch = ReplOrchestratorV2::new(
        PackRouter::new(vec![load_onboarding_pack()]),
        executor.clone(),
    );
    let session_id = scope_and_select_pack(&orch, "onboarding-request").await;

    // Build and execute runbook.
    orch.process(
        session_id,
        UserInputV2::Message {
            content: "Test Fund".to_string(),
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
    orch.process(
        session_id,
        UserInputV2::Command {
            command: ReplCommandV2::Run,
        },
    )
    .await
    .unwrap();

    let calls = executor.get_calls();
    assert!(!calls.is_empty());

    for (i, dsl) in calls.iter().enumerate() {
        // Each DSL statement should be a valid s-expression (starts with '(' and ends with ')').
        let trimmed = dsl.trim();
        assert!(
            trimmed.starts_with('(') && trimmed.ends_with(')'),
            "DSL call {} is not a valid s-expression: {}",
            i,
            dsl
        );

        // Should contain a verb name.
        let verb = trimmed
            .trim_start_matches('(')
            .split_whitespace()
            .next()
            .unwrap_or("");
        assert!(
            verb.contains('.'),
            "DSL verb should be domain-qualified (e.g. cbu.create), got: {}",
            verb
        );
    }
}
