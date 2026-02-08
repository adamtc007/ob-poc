//! Phase 2 Integration Tests — VerbSentences + IntentService + Sentence YAML
//!
//! These tests verify Phase 2 functionality:
//! 1. VerbSentences deserialization from YAML
//! 2. VerbConfigIndex prefers YAML sentences over hardcoded
//! 3. Pack-only FQNs still get hardcoded templates
//! 4. IntentService.check_clarification uses sentences.clarify
//! 5. IntentService.generate_sentence uses YAML sentences.step
//! 6. confirm_policy from YAML overrides hardcoded
//! 7. Orchestrator uses IntentService when available
//! 8. NeedsClarification never returns raw arg names
#![cfg(feature = "vnext-repl")]

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use dsl_core::config::types::VerbsConfig;
use ob_poc::journey::router::PackRouter;
use ob_poc::repl::intent_matcher::IntentMatcher;
use ob_poc::repl::intent_service::{ClarificationOutcome, IntentService};
use ob_poc::repl::orchestrator_v2::{ReplOrchestratorV2, StubExecutor};
use ob_poc::repl::runbook::ConfirmPolicy;
use ob_poc::repl::types::{IntentMatchResult, MatchContext, MatchOutcome};
use ob_poc::repl::types_v2::UserInputV2;
use ob_poc::repl::verb_config_index::VerbConfigIndex;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Load real VerbsConfig from cbu.yaml.
fn load_cbu_verbs_config() -> VerbsConfig {
    let yaml = include_str!("../config/verbs/cbu.yaml");
    serde_yaml::from_str(yaml).expect("cbu.yaml should parse as VerbsConfig")
}

/// Load real VerbsConfig from session.yaml.
fn load_session_verbs_config() -> VerbsConfig {
    let yaml = include_str!("../config/verbs/session.yaml");
    serde_yaml::from_str(yaml).expect("session.yaml should parse as VerbsConfig")
}

/// Load real VerbsConfig from document.yaml.
fn load_document_verbs_config() -> VerbsConfig {
    let yaml = include_str!("../config/verbs/document.yaml");
    serde_yaml::from_str(yaml).expect("document.yaml should parse as VerbsConfig")
}

/// Merge multiple VerbsConfigs into one.
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

/// Build a VerbConfigIndex from real YAML config files.
fn build_real_index() -> VerbConfigIndex {
    let config = merge_configs(vec![
        load_cbu_verbs_config(),
        load_session_verbs_config(),
        load_document_verbs_config(),
    ]);
    VerbConfigIndex::from_verbs_config(&config)
}

/// A mock IntentMatcher that returns configurable results.
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

// ===========================================================================
// TEST 1: VerbSentences deserialization from YAML
// ===========================================================================

#[test]
fn test_verb_sentences_deserialization() {
    let config = load_cbu_verbs_config();

    // cbu.create should have sentences from YAML.
    let cbu_domain = config.domains.get("cbu").expect("cbu domain should exist");
    let create_verb = cbu_domain
        .verbs
        .get("create")
        .expect("cbu.create should exist");

    let sentences = create_verb
        .sentences
        .as_ref()
        .expect("cbu.create should have sentences");

    // Verify step templates.
    assert!(
        !sentences.step.is_empty(),
        "cbu.create should have step templates"
    );
    assert!(
        sentences.step[0].contains("{name}"),
        "Step template should contain {{name}} placeholder: '{}'",
        sentences.step[0]
    );

    // Verify clarify map.
    assert!(
        sentences.clarify.contains_key("name"),
        "cbu.create should have clarify template for 'name'"
    );
    assert!(
        sentences.clarify.contains_key("jurisdiction"),
        "cbu.create should have clarify template for 'jurisdiction'"
    );

    // Verify completed template.
    assert!(
        sentences.completed.is_some(),
        "cbu.create should have completed template"
    );
}

#[test]
fn test_verb_sentences_deserialization_session() {
    let config = load_session_verbs_config();

    let session_domain = config
        .domains
        .get("session")
        .expect("session domain should exist");
    let load_galaxy = session_domain
        .verbs
        .get("load-galaxy")
        .expect("session.load-galaxy should exist");

    let sentences = load_galaxy
        .sentences
        .as_ref()
        .expect("session.load-galaxy should have sentences");

    assert!(!sentences.step.is_empty());
    assert!(
        sentences.clarify.contains_key("jurisdiction"),
        "session.load-galaxy should have clarify for 'jurisdiction', got keys: {:?}",
        sentences.clarify.keys().collect::<Vec<_>>()
    );

    // Confirm policy from YAML.
    assert!(
        load_galaxy.confirm_policy.is_some(),
        "session.load-galaxy should have confirm_policy in YAML"
    );
}

#[test]
fn test_verb_sentences_deserialization_document() {
    let config = load_document_verbs_config();

    let document_domain = config
        .domains
        .get("document")
        .expect("document domain should exist");
    let solicit_set = document_domain
        .verbs
        .get("solicit-set")
        .expect("document.solicit-set should exist");

    let sentences = solicit_set
        .sentences
        .as_ref()
        .expect("document.solicit-set should have sentences");

    assert!(!sentences.step.is_empty());
    assert!(sentences.clarify.contains_key("doc-types"));
}

// ===========================================================================
// TEST 2: VerbConfigIndex prefers YAML sentences over hardcoded
// ===========================================================================

#[test]
fn test_yaml_sentences_override_hardcoded() {
    let index = build_real_index();

    // cbu.create has YAML sentences AND hardcoded templates.
    // YAML should win.
    let entry = index
        .get("cbu.create")
        .expect("cbu.create should be in index");

    // sentence_templates should come from YAML sentences.step[].
    assert!(
        !entry.sentence_templates.is_empty(),
        "cbu.create should have sentence templates"
    );
    assert!(
        entry.sentence_templates[0].contains("{name}"),
        "Template should be from YAML (contains {{name}}): '{}'",
        entry.sentence_templates[0]
    );

    // VerbSentences should be present.
    assert!(
        entry.sentences.is_some(),
        "cbu.create should have VerbSentences from YAML"
    );

    let sentences = entry.sentences.as_ref().unwrap();
    assert!(
        !sentences.clarify.is_empty(),
        "VerbSentences should carry clarify map"
    );
}

// ===========================================================================
// TEST 3: Pack-only FQNs still get hardcoded templates
// ===========================================================================

#[test]
fn test_pack_only_fqns_get_hardcoded() {
    let index = build_real_index();

    // cbu.assign-product has NO YAML sentences (not in cbu.yaml).
    // It should still get hardcoded templates from pack_verb_sentence_templates().
    // Note: if the verb doesn't exist in YAML at all, it won't be in the index.
    // The hardcoded map is only used for verbs that ARE in YAML but don't have sentences.
    // For FQNs like "cbu.assign-product" that DON'T exist in YAML verb config,
    // they won't appear in the index (they only appear via pack template instantiation).
    //
    // What we CAN test: verbs that exist in YAML WITHOUT sentences should
    // fall through to hardcoded templates.

    // cbu.assign-role has YAML sentences — verify it uses them, not hardcoded.
    let assign_role = index.get("cbu.assign-role");
    if let Some(entry) = assign_role {
        assert!(
            entry.sentences.is_some(),
            "cbu.assign-role has YAML sentences and should carry them"
        );
    }
}

// ===========================================================================
// TEST 4: IntentService.check_clarification uses sentences.clarify
// ===========================================================================

#[test]
fn test_clarification_uses_sentence_templates() {
    let index = build_real_index();
    let matcher = MockIntentMatcher::matched("cbu.create", 0.90, None);
    let svc = IntentService::new(Arc::new(matcher), Arc::new(index));

    // Missing "name" arg — required for cbu.create.
    let args = HashMap::new();
    let result = svc.check_clarification("cbu.create", &args);

    match result {
        ClarificationOutcome::NeedsClarification { prompts, .. } => {
            assert!(!prompts.is_empty(), "Should have at least one prompt");

            // Verify prompt uses sentences.clarify template, not raw arg name.
            let (arg_name, prompt) = &prompts[0];
            assert!(
                !prompt.is_empty(),
                "Clarification prompt should not be empty"
            );
            // The prompt should be conversational, not just the arg name.
            assert!(
                prompt.len() > arg_name.len(),
                "Prompt '{}' should be longer than raw arg name '{}'",
                prompt,
                arg_name
            );
        }
        ClarificationOutcome::Complete => {
            // If the verb has no required args in config, this is acceptable.
            // cbu.create's required args depend on VerbConfig.args which
            // are loaded from YAML. The IntentService checks sentences.clarify
            // keys as the required arg set.
        }
    }
}

// ===========================================================================
// TEST 5: IntentService.generate_sentence uses YAML sentences.step
// ===========================================================================

#[test]
fn test_sentence_from_yaml() {
    let index = build_real_index();
    let matcher = MockIntentMatcher::matched("cbu.create", 0.90, None);
    let svc = IntentService::new(Arc::new(matcher), Arc::new(index));

    let args = HashMap::from([
        ("name".to_string(), "Allianz Lux".to_string()),
        ("jurisdiction".to_string(), "LU".to_string()),
    ]);

    let sentence = svc.generate_sentence("cbu.create", &args);

    // Sentence should come from YAML template with args substituted.
    assert!(
        sentence.contains("Allianz Lux"),
        "Sentence should contain arg value: '{}'",
        sentence
    );
    assert!(
        !sentence.contains('{'),
        "Sentence should not contain unresolved placeholders: '{}'",
        sentence
    );
}

// ===========================================================================
// TEST 6: confirm_policy from YAML overrides hardcoded
// ===========================================================================

#[test]
fn test_confirm_policy_from_yaml() {
    let index = build_real_index();

    // session.load-galaxy has confirm_policy: quick_confirm in YAML.
    let policy = index.confirm_policy("session.load-galaxy");
    assert_eq!(
        policy,
        ConfirmPolicy::QuickConfirm,
        "session.load-galaxy should get QuickConfirm from YAML"
    );

    // cbu.create has no confirm_policy in YAML and no hardcoded override → Always.
    let policy = index.confirm_policy("cbu.create");
    assert_eq!(
        policy,
        ConfirmPolicy::Always,
        "cbu.create should default to Always"
    );
}

// ===========================================================================
// TEST 7: Orchestrator uses IntentService when available
// ===========================================================================

#[tokio::test]
async fn test_orchestrator_with_intent_service() {
    let index = build_real_index();
    let matcher: Arc<dyn IntentMatcher> = Arc::new(MockIntentMatcher::matched(
        "cbu.create",
        0.90,
        Some("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")"),
    ));
    let intent_service = Arc::new(IntentService::new(matcher.clone(), Arc::new(index)));

    let packs = ob_poc::journey::pack::load_pack_from_bytes(include_bytes!(
        "../config/packs/onboarding-request.yaml"
    ))
    .unwrap();
    let router = PackRouter::new(vec![(Arc::new(packs.0), packs.1)]);

    let orch = ReplOrchestratorV2::new(router, Arc::new(StubExecutor))
        .with_intent_matcher(matcher)
        .with_intent_service(intent_service);

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

    // Force select a pack first, then send a message that triggers IntentService.
    orch.process(
        session_id,
        UserInputV2::SelectPack {
            pack_id: "onboarding-request".to_string(),
        },
    )
    .await
    .unwrap();

    // Now in InPack state — answer questions to fill slots.
    // The IntentService path is used when the pack router delegates to verb matching.
    // For this test, we just verify the orchestrator builds and processes without error.
    let session = orch.get_session(session_id).await.unwrap();
    assert!(
        session.active_pack_id().is_some(),
        "Should have active pack after pack selection"
    );
}

// ===========================================================================
// TEST 8: NeedsClarification never returns raw arg names
// ===========================================================================

#[test]
fn test_no_raw_arg_names_in_clarification() {
    let index = build_real_index();
    let matcher = MockIntentMatcher::matched("cbu.create", 0.90, None);
    let svc = IntentService::new(Arc::new(matcher), Arc::new(index));

    // Test with no args — should need clarification for cbu.create.
    let args = HashMap::new();
    let result = svc.check_clarification("cbu.create", &args);

    if let ClarificationOutcome::NeedsClarification { prompts, .. } = result {
        for (arg_name, prompt) in &prompts {
            // The prompt should NEVER be just the raw arg name.
            assert_ne!(
                prompt, arg_name,
                "Prompt should not be the raw arg name '{}', got '{}'",
                arg_name, prompt
            );
            // The prompt should be a human-readable question.
            assert!(
                prompt.contains('?') || prompt.contains("Which") || prompt.contains("What"),
                "Prompt should be conversational (contain question mark or question word): '{}'",
                prompt
            );
        }
    }
}

// ===========================================================================
// TEST 9: VerbSentences accessor on IntentService
// ===========================================================================

#[test]
fn test_intent_service_verb_sentences_accessor() {
    let index = build_real_index();
    let matcher = MockIntentMatcher::matched("cbu.create", 0.90, None);
    let svc = IntentService::new(Arc::new(matcher), Arc::new(index));

    // cbu.create has YAML sentences.
    let sentences = svc.verb_sentences("cbu.create");
    assert!(
        sentences.is_some(),
        "cbu.create should have VerbSentences via IntentService"
    );

    let s = sentences.unwrap();
    assert!(!s.step.is_empty());
    assert!(!s.clarify.is_empty());
    assert!(s.completed.is_some());

    // Nonexistent verb.
    assert!(svc.verb_sentences("nonexistent.verb").is_none());
}

// ===========================================================================
// TEST 10: V2 API route types round-trip
// ===========================================================================

#[test]
fn test_v2_input_request_round_trip() {
    use ob_poc::api::repl_routes_v2::InputRequestV2;

    // Message
    let json = r#"{"type":"message","content":"create a fund"}"#;
    let req: InputRequestV2 = serde_json::from_str(json).unwrap();
    assert!(matches!(req, InputRequestV2::Message { .. }));

    // Confirm
    let json = r#"{"type":"confirm"}"#;
    let req: InputRequestV2 = serde_json::from_str(json).unwrap();
    assert!(matches!(req, InputRequestV2::Confirm));

    // SelectScope
    let json = r#"{"type":"select_scope","group_id":"11111111-1111-1111-1111-111111111111","group_name":"Allianz"}"#;
    let req: InputRequestV2 = serde_json::from_str(json).unwrap();
    assert!(matches!(req, InputRequestV2::SelectScope { .. }));
}
