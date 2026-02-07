//! IntentService — Unified 5-phase pipeline for the v2 REPL.
//!
//! Composes existing services (IntentMatcher, SentenceGenerator,
//! VerbConfigIndex) into a clean API that the orchestrator calls
//! phase-by-phase.
//!
//! # Phases
//!
//! 1. **Input** — Raw user text arrives
//! 2. **Verb matching** — `match_verb()` delegates to `IntentMatcher`
//! 3. **Clarification** — `check_clarification()` checks missing args against `sentences.clarify`
//! 4. **Sentence generation** — `generate_sentence()` deterministic template substitution
//! 5. **Confirmation** — `confirm_policy()` determines if user must confirm

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use super::context_stack::ContextStack;
use super::intent_matcher::IntentMatcher;
use super::runbook::ConfirmPolicy;
use super::sentence_gen::SentenceGenerator;
use super::types::{IntentMatchResult, MatchContext, MatchOutcome};
use super::verb_config_index::VerbConfigIndex;
use dsl_core::config::types::VerbSentences;

// ============================================================================
// Outcome Types
// ============================================================================

/// Outcome of verb matching (simplified view for orchestrator).
#[derive(Debug, Clone)]
pub enum VerbMatchOutcome {
    /// Clear winner found.
    Matched {
        verb: String,
        confidence: f32,
        generated_dsl: Option<String>,
    },
    /// Multiple verbs matched — need user to pick.
    Ambiguous {
        candidates: Vec<VerbMatchCandidate>,
        margin: f32,
    },
    /// No matching verb found.
    NoMatch { reason: String },
    /// Input is direct DSL (bypass matching).
    DirectDsl { source: String },
    /// Scope selection needed before verb matching can proceed.
    NeedsScopeSelection,
    /// Entity references need resolution.
    NeedsEntityResolution,
    /// Full intent result for cases not covered above (intent tier, client group, etc.).
    Other(Box<IntentMatchResult>),
}

/// A verb candidate for disambiguation.
#[derive(Debug, Clone)]
pub struct VerbMatchCandidate {
    pub verb_fqn: String,
    pub description: String,
    pub score: f32,
}

/// Outcome of arg clarification check.
#[derive(Debug, Clone)]
pub enum ClarificationOutcome {
    /// All required args are present.
    Complete,
    /// Missing args — return conversational prompts.
    NeedsClarification {
        missing_args: Vec<String>,
        /// (arg_name, clarify_prompt) pairs.
        prompts: Vec<(String, String)>,
    },
}

// ============================================================================
// IntentService
// ============================================================================

/// Unified facade composing IntentMatcher + SentenceGenerator + VerbConfigIndex.
///
/// The orchestrator calls methods on IntentService instead of managing
/// three separate services directly.
pub struct IntentService {
    intent_matcher: Arc<dyn IntentMatcher>,
    sentence_gen: SentenceGenerator,
    verb_config_index: Arc<VerbConfigIndex>,
}

impl IntentService {
    pub fn new(
        intent_matcher: Arc<dyn IntentMatcher>,
        verb_config_index: Arc<VerbConfigIndex>,
    ) -> Self {
        Self {
            intent_matcher,
            sentence_gen: SentenceGenerator,
            verb_config_index,
        }
    }

    /// Phase 2: Verb matching (delegates to IntentMatcher).
    ///
    /// Maps the full `IntentMatchResult` to a simplified `VerbMatchOutcome`
    /// that the orchestrator can pattern-match on.
    pub async fn match_verb(&self, input: &str, ctx: &MatchContext) -> Result<VerbMatchOutcome> {
        let result = self.intent_matcher.match_intent(input, ctx).await?;

        let outcome = match &result.outcome {
            MatchOutcome::Matched { verb, confidence } => VerbMatchOutcome::Matched {
                verb: verb.clone(),
                confidence: *confidence,
                generated_dsl: result.generated_dsl.clone(),
            },
            MatchOutcome::Ambiguous { margin } => {
                let candidates = result
                    .verb_candidates
                    .iter()
                    .map(|vc| VerbMatchCandidate {
                        verb_fqn: vc.verb_fqn.clone(),
                        description: vc.description.clone(),
                        score: vc.score,
                    })
                    .collect();
                VerbMatchOutcome::Ambiguous {
                    candidates,
                    margin: *margin,
                }
            }
            MatchOutcome::NoMatch { reason } => VerbMatchOutcome::NoMatch {
                reason: reason.clone(),
            },
            MatchOutcome::DirectDsl { source } => VerbMatchOutcome::DirectDsl {
                source: source.clone(),
            },
            MatchOutcome::NeedsScopeSelection => VerbMatchOutcome::NeedsScopeSelection,
            MatchOutcome::NeedsEntityResolution => VerbMatchOutcome::NeedsEntityResolution,
            // Intent tier, client group, etc. — pass through as Other
            _ => VerbMatchOutcome::Other(Box::new(result)),
        };

        Ok(outcome)
    }

    /// Phase 2b: Context-aware verb matching with pack scoring.
    ///
    /// Uses `search_with_context()` on the IntentMatcher trait which:
    /// 1. Runs raw semantic search via `match_intent()`
    /// 2. Applies pack scoring (boost in-pack, penalise out-of-pack, zero forbidden)
    /// 3. Applies ambiguity policy (Invariant I-5)
    ///
    /// This is the primary verb matching path when a ContextStack is available.
    /// Falls back to `match_verb()` semantics when no pack is active.
    pub async fn match_verb_with_context(
        &self,
        input: &str,
        ctx: &MatchContext,
        stack: &ContextStack,
    ) -> Result<VerbMatchOutcome> {
        let result = self
            .intent_matcher
            .search_with_context(input, ctx, stack)
            .await?;

        let outcome = match &result.outcome {
            MatchOutcome::Matched { verb, confidence } => VerbMatchOutcome::Matched {
                verb: verb.clone(),
                confidence: *confidence,
                generated_dsl: result.generated_dsl.clone(),
            },
            MatchOutcome::Ambiguous { margin } => {
                let candidates = result
                    .verb_candidates
                    .iter()
                    .map(|vc| VerbMatchCandidate {
                        verb_fqn: vc.verb_fqn.clone(),
                        description: vc.description.clone(),
                        score: vc.score,
                    })
                    .collect();
                VerbMatchOutcome::Ambiguous {
                    candidates,
                    margin: *margin,
                }
            }
            MatchOutcome::NoMatch { reason } => VerbMatchOutcome::NoMatch {
                reason: reason.clone(),
            },
            MatchOutcome::DirectDsl { source } => VerbMatchOutcome::DirectDsl {
                source: source.clone(),
            },
            MatchOutcome::NeedsScopeSelection => VerbMatchOutcome::NeedsScopeSelection,
            MatchOutcome::NeedsEntityResolution => VerbMatchOutcome::NeedsEntityResolution,
            _ => VerbMatchOutcome::Other(Box::new(result)),
        };

        Ok(outcome)
    }

    /// Phase 3: Check if matched verb needs arg clarification.
    ///
    /// Uses `sentences.clarify` templates instead of raw arg names.
    /// Only checks args that have clarify prompts defined — this is a
    /// UX enhancement, not a validation gate.
    pub fn check_clarification(
        &self,
        verb: &str,
        provided_args: &HashMap<String, String>,
    ) -> ClarificationOutcome {
        let sentences = match self.verb_config_index.verb_sentences(verb) {
            Some(s) if !s.clarify.is_empty() => s,
            _ => return ClarificationOutcome::Complete,
        };

        let mut missing_args = Vec::new();
        let mut prompts = Vec::new();

        // Check each clarify-configured arg against provided args
        for (arg_name, prompt) in &sentences.clarify {
            if !provided_args.contains_key(arg_name) {
                // Also check if the verb's arg definition says it's required
                if let Some(entry) = self.verb_config_index.get(verb) {
                    let is_required = entry.args.iter().any(|a| a.name == *arg_name && a.required);
                    if is_required {
                        missing_args.push(arg_name.clone());
                        prompts.push((arg_name.clone(), prompt.clone()));
                    }
                }
            }
        }

        if missing_args.is_empty() {
            ClarificationOutcome::Complete
        } else {
            ClarificationOutcome::NeedsClarification {
                missing_args,
                prompts,
            }
        }
    }

    /// Phase 4: Sentence generation (deterministic, no LLM).
    ///
    /// Uses sentence templates with priority:
    /// 1. VerbConfigIndex sentence_templates (YAML > hardcoded)
    /// 2. SentenceGenerator fallback (invocation_phrases > phrase_gen > structured)
    pub fn generate_sentence(&self, verb: &str, args: &HashMap<String, String>) -> String {
        let templates = self.verb_config_index.sentence_templates(verb);
        let phrases = self.verb_config_index.invocation_phrases(verb);
        let description = self.verb_config_index.description(verb);

        // If we have sentence templates, try those first via SentenceGenerator
        if !templates.is_empty() {
            self.sentence_gen
                .generate(verb, args, templates, description)
        } else {
            self.sentence_gen.generate(verb, args, phrases, description)
        }
    }

    /// Get confirm policy for a verb.
    pub fn confirm_policy(&self, verb: &str) -> ConfirmPolicy {
        self.verb_config_index.confirm_policy(verb)
    }

    /// Get full VerbSentences for a verb (if available).
    pub fn verb_sentences(&self, verb: &str) -> Option<&VerbSentences> {
        self.verb_config_index.verb_sentences(verb)
    }

    /// Access to the underlying IntentMatcher (for advanced use).
    pub fn intent_matcher(&self) -> &dyn IntentMatcher {
        self.intent_matcher.as_ref()
    }

    /// Access to the underlying VerbConfigIndex.
    pub fn verb_config_index(&self) -> &VerbConfigIndex {
        &self.verb_config_index
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::types::{EntityMention, VerbCandidate};
    use super::*;
    use async_trait::async_trait;
    use dsl_core::config::types::{ArgConfig, ArgType};

    /// Stub IntentMatcher for testing IntentService without real verb search.
    struct StubIntentMatcher {
        result: IntentMatchResult,
    }

    impl StubIntentMatcher {
        fn matched(verb: &str, confidence: f32, dsl: Option<&str>) -> Self {
            Self {
                result: IntentMatchResult {
                    outcome: MatchOutcome::Matched {
                        verb: verb.to_string(),
                        confidence,
                    },
                    verb_candidates: vec![VerbCandidate {
                        verb_fqn: verb.to_string(),
                        description: format!("Test verb {}", verb),
                        score: confidence,
                        example: None,
                        domain: None,
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
                        .map(|(fqn, score)| VerbCandidate {
                            verb_fqn: fqn.to_string(),
                            description: format!("Test {}", fqn),
                            score,
                            example: None,
                            domain: None,
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

        fn no_match(reason: &str) -> Self {
            Self {
                result: IntentMatchResult {
                    outcome: MatchOutcome::NoMatch {
                        reason: reason.to_string(),
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
    impl IntentMatcher for StubIntentMatcher {
        async fn match_intent(
            &self,
            _utterance: &str,
            _context: &MatchContext,
        ) -> Result<IntentMatchResult> {
            Ok(self.result.clone())
        }
    }

    fn make_test_arg(name: &str, required: bool) -> ArgConfig {
        ArgConfig {
            name: name.to_string(),
            arg_type: ArgType::String,
            required,
            maps_to: None,
            lookup: None,
            valid_values: None,
            default: None,
            description: None,
            validation: None,
            fuzzy_check: None,
            slot_type: None,
            preferred_roles: vec![],
        }
    }

    fn make_test_index_with_sentences() -> VerbConfigIndex {
        use dsl_core::config::types::{DomainConfig, VerbBehavior, VerbConfig, VerbsConfig};

        let mut domains = HashMap::new();
        let mut cbu_verbs = HashMap::new();

        cbu_verbs.insert(
            "create".to_string(),
            VerbConfig {
                description: "Create a new CBU".to_string(),
                behavior: VerbBehavior::Plugin,
                args: vec![
                    make_test_arg("name", true),
                    make_test_arg("jurisdiction", true),
                ],
                sentences: Some(VerbSentences {
                    step: vec!["Create {name} structure in {jurisdiction}".to_string()],
                    summary: vec!["created {name}".to_string()],
                    clarify: {
                        let mut m = HashMap::new();
                        m.insert(
                            "name".to_string(),
                            "What should the structure be called?".to_string(),
                        );
                        m.insert(
                            "jurisdiction".to_string(),
                            "Which jurisdiction?".to_string(),
                        );
                        m
                    },
                    completed: Some("{name} structure created in {jurisdiction}".to_string()),
                }),
                crud: None,
                handler: None,
                graph_query: None,
                returns: None,
                produces: None,
                consumes: vec![],
                lifecycle: None,
                metadata: None,
                invocation_phrases: vec![],
                policy: None,
                confirm_policy: None,
            },
        );

        domains.insert(
            "cbu".to_string(),
            DomainConfig {
                description: "CBU ops".to_string(),
                verbs: cbu_verbs,
                dynamic_verbs: vec![],
                invocation_hints: vec![],
            },
        );

        let config = VerbsConfig {
            version: "1.0".to_string(),
            domains,
        };

        VerbConfigIndex::from_verbs_config(&config)
    }

    fn default_context() -> MatchContext {
        MatchContext {
            client_group_id: None,
            client_group_name: None,
            scope: None,
            dominant_entity_id: None,
            user_id: None,
            domain_hint: None,
            bindings: vec![],
        }
    }

    #[tokio::test]
    async fn test_match_verb_matched() {
        let matcher =
            StubIntentMatcher::matched("cbu.create", 0.92, Some("(cbu.create :name \"Test\")"));
        let index = make_test_index_with_sentences();
        let svc = IntentService::new(Arc::new(matcher), Arc::new(index));

        let outcome = svc
            .match_verb("create a fund", &default_context())
            .await
            .unwrap();
        match outcome {
            VerbMatchOutcome::Matched {
                verb,
                confidence,
                generated_dsl,
            } => {
                assert_eq!(verb, "cbu.create");
                assert!(confidence > 0.9);
                assert!(generated_dsl.is_some());
            }
            other => panic!("Expected Matched, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_match_verb_ambiguous() {
        let matcher = StubIntentMatcher::ambiguous(
            vec![("session.load-galaxy", 0.82), ("session.load-system", 0.79)],
            0.03,
        );
        let index = make_test_index_with_sentences();
        let svc = IntentService::new(Arc::new(matcher), Arc::new(index));

        let outcome = svc
            .match_verb("load the book", &default_context())
            .await
            .unwrap();
        match outcome {
            VerbMatchOutcome::Ambiguous { candidates, margin } => {
                assert_eq!(candidates.len(), 2);
                assert!(margin < 0.05);
            }
            other => panic!("Expected Ambiguous, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_match_verb_no_match() {
        let matcher = StubIntentMatcher::no_match("below threshold");
        let index = make_test_index_with_sentences();
        let svc = IntentService::new(Arc::new(matcher), Arc::new(index));

        let outcome = svc
            .match_verb("asdfghjkl", &default_context())
            .await
            .unwrap();
        match outcome {
            VerbMatchOutcome::NoMatch { reason } => {
                assert!(reason.contains("threshold"));
            }
            other => panic!("Expected NoMatch, got {:?}", other),
        }
    }

    #[test]
    fn test_check_clarification_complete() {
        let index = make_test_index_with_sentences();
        let svc = IntentService::new(Arc::new(StubIntentMatcher::no_match("")), Arc::new(index));

        let mut args = HashMap::new();
        args.insert("name".to_string(), "Allianz Fund".to_string());
        args.insert("jurisdiction".to_string(), "LU".to_string());

        match svc.check_clarification("cbu.create", &args) {
            ClarificationOutcome::Complete => {} // expected
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn test_check_clarification_missing_required_arg() {
        let index = make_test_index_with_sentences();
        let svc = IntentService::new(Arc::new(StubIntentMatcher::no_match("")), Arc::new(index));

        // Only provide name, missing jurisdiction
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Allianz Fund".to_string());

        match svc.check_clarification("cbu.create", &args) {
            ClarificationOutcome::NeedsClarification {
                missing_args,
                prompts,
            } => {
                assert!(missing_args.contains(&"jurisdiction".to_string()));
                // Prompt should be the human-readable clarify text, NOT the raw arg name
                let (arg, prompt) = &prompts[0];
                assert_eq!(arg, "jurisdiction");
                assert_eq!(prompt, "Which jurisdiction?");
            }
            other => panic!("Expected NeedsClarification, got {:?}", other),
        }
    }

    #[test]
    fn test_check_clarification_no_raw_arg_names() {
        // Quality gate: NeedsClarification prompts must NEVER be raw arg names
        let index = make_test_index_with_sentences();
        let svc = IntentService::new(Arc::new(StubIntentMatcher::no_match("")), Arc::new(index));

        let args = HashMap::new(); // no args at all

        match svc.check_clarification("cbu.create", &args) {
            ClarificationOutcome::NeedsClarification { prompts, .. } => {
                for (arg_name, prompt) in &prompts {
                    // Prompt must NOT be identical to the raw arg name
                    assert_ne!(
                        arg_name, prompt,
                        "Clarification prompt for '{}' is just the raw arg name",
                        arg_name
                    );
                    // Prompt should contain natural language (at least one space or ?)
                    assert!(
                        prompt.contains(' ') || prompt.contains('?'),
                        "Prompt '{}' for arg '{}' doesn't look like natural language",
                        prompt,
                        arg_name
                    );
                }
            }
            ClarificationOutcome::Complete => {
                // If verb has no required args with clarify, this is OK
            }
        }
    }

    #[test]
    fn test_generate_sentence_uses_yaml_templates() {
        let index = make_test_index_with_sentences();
        let svc = IntentService::new(Arc::new(StubIntentMatcher::no_match("")), Arc::new(index));

        let mut args = HashMap::new();
        args.insert("name".to_string(), "Allianz Fund".to_string());
        args.insert("jurisdiction".to_string(), "LU".to_string());

        let sentence = svc.generate_sentence("cbu.create", &args);
        assert!(
            sentence.contains("Allianz Fund"),
            "Sentence should contain arg value: {}",
            sentence
        );
        assert!(
            sentence.contains("LU"),
            "Sentence should contain arg value: {}",
            sentence
        );
    }

    #[test]
    fn test_confirm_policy() {
        let index = make_test_index_with_sentences();
        let svc = IntentService::new(Arc::new(StubIntentMatcher::no_match("")), Arc::new(index));

        // Default policy
        assert_eq!(svc.confirm_policy("cbu.create"), ConfirmPolicy::Always);
        // Nonexistent verb
        assert_eq!(
            svc.confirm_policy("nonexistent.verb"),
            ConfirmPolicy::Always
        );
    }

    #[test]
    fn test_verb_sentences_accessor() {
        let index = make_test_index_with_sentences();
        let svc = IntentService::new(Arc::new(StubIntentMatcher::no_match("")), Arc::new(index));

        let sentences = svc.verb_sentences("cbu.create").unwrap();
        assert!(!sentences.step.is_empty());
        assert!(!sentences.clarify.is_empty());
        assert!(sentences.completed.is_some());

        assert!(svc.verb_sentences("nonexistent.verb").is_none());
    }
}
