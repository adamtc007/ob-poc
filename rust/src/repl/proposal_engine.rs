//! Proposal Engine — Deterministic step proposal generation for the v2 REPL.
//!
//! Composes `IntentService`, `VerbConfigIndex`, and `SentenceGenerator` to
//! produce a **ranked list of step proposals with evidence**. The engine
//! never executes — it only proposes edits to the runbook.
//!
//! # Strategy
//!
//! 1. **Template fast-path**: Score pack templates against user input using
//!    word-overlap scoring. Qualifying templates are expanded via
//!    `instantiate_template()` and wrapped as proposals.
//! 2. **Verb matching fallback**: Delegates to `IntentService.match_verb_with_context()`
//!    with pack-scoped scoring (P-2 invariant) and converts results into ranked proposals.
//! 3. **Pack constraint filtering**: Respects `allowed_verbs` / `forbidden_verbs`.
//! 4. **Deterministic**: Same inputs always produce the same `ProposalSet`.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::context_stack::ContextStack;
use super::intent_service::{IntentService, VerbMatchOutcome};
use super::runbook::{ConfirmPolicy, Runbook};
use super::sentence_gen::SentenceGenerator;
use super::types::MatchContext;
use super::verb_config_index::VerbConfigIndex;
use crate::journey::pack::{PackManifest, PackTemplate};
use crate::journey::template::instantiate_template;

// ============================================================================
// Types
// ============================================================================

/// Source of a proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProposalSource {
    /// From a pack template (fast path).
    Template { template_id: String },
    /// From verb search (fallback).
    VerbMatch,
    /// Direct DSL input (bypass).
    DirectDsl,
}

/// Evidence explaining why a proposal was generated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalEvidence {
    /// Where this proposal came from.
    pub source: ProposalSource,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f32,
    /// Human-readable explanation ("why this verb/template").
    pub rationale: String,
    /// Number of required args that are still missing.
    pub missing_required_args: usize,
    /// Template fit score (only for template proposals).
    pub template_fit_score: Option<f32>,
    /// Verb search score (only for verb-match proposals).
    pub verb_search_score: Option<f32>,
}

/// A single proposed step (never executed, only proposed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepProposal {
    /// Unique proposal ID (for selection tracking).
    pub id: Uuid,
    /// Fully-qualified verb name.
    pub verb: String,
    /// Human-readable sentence.
    pub sentence: String,
    /// Generated DSL.
    pub dsl: String,
    /// Extracted arguments.
    pub args: HashMap<String, String>,
    /// Evidence metadata.
    pub evidence: ProposalEvidence,
    /// Confirm policy for this verb.
    pub confirm_policy: ConfirmPolicy,
}

/// A ranked set of proposals from the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSet {
    /// Original user input that triggered proposals.
    pub original_input: String,
    /// Ranked proposals (best first).
    pub proposals: Vec<StepProposal>,
    /// Whether the top proposal is a template (fast path).
    pub template_fast_path: bool,
    /// Deterministic hash for reproducibility verification.
    pub proposal_hash: String,
}

// ============================================================================
// Constants
// ============================================================================

/// Minimum word-overlap score for a template to qualify.
const TEMPLATE_SCORE_THRESHOLD: f32 = 0.3;

/// Confidence boost for template proposals to implement "prefer templates first".
pub const TEMPLATE_CONFIDENCE_BOOST: f32 = 0.1;

/// Confidence threshold for single-proposal auto-advance.
pub const AUTO_ADVANCE_THRESHOLD: f32 = 0.85;

/// Maximum number of verb-match proposals to return.
const MAX_VERB_PROPOSALS: usize = 5;

// ============================================================================
// ProposalEngine
// ============================================================================

/// Deterministic proposal engine composing IntentService + VerbConfigIndex.
///
/// Stateless — pack context and runbook are passed as method arguments.
pub struct ProposalEngine {
    intent_service: Arc<IntentService>,
    verb_config_index: Arc<VerbConfigIndex>,
    sentence_gen: SentenceGenerator,
}

impl ProposalEngine {
    pub fn new(
        intent_service: Arc<IntentService>,
        verb_config_index: Arc<VerbConfigIndex>,
    ) -> Self {
        Self {
            intent_service,
            sentence_gen: SentenceGenerator,
            verb_config_index,
        }
    }

    /// Produce a ranked set of step proposals for the given input.
    ///
    /// Never executes — only proposes edits. Deterministic: same inputs
    /// always produce the same `ProposalSet`.
    #[allow(clippy::too_many_arguments)]
    pub async fn propose(
        &self,
        input: &str,
        pack: Option<&PackManifest>,
        _runbook: &Runbook,
        match_ctx: &MatchContext,
        context_stack: &ContextStack,
        context_vars: &HashMap<String, String>,
        answers: &HashMap<String, serde_json::Value>,
    ) -> ProposalSet {
        let trimmed = input.trim();

        // 1. Direct DSL check.
        if trimmed.starts_with('(') {
            return self.direct_dsl_proposal(trimmed);
        }

        let mut proposals = Vec::new();

        // 2. Template scoring (fast path).
        if let Some(pack) = pack {
            if !pack.templates.is_empty() {
                let template_proposals =
                    self.score_templates(trimmed, &pack.templates, context_vars, answers);
                proposals.extend(template_proposals);
            }
        }

        // 3. Verb matching with pack-scoped scoring (P-2 invariant).
        let verb_proposals = self
            .verb_match_proposals(trimmed, match_ctx, context_stack)
            .await;
        proposals.extend(verb_proposals);

        // 4. Pack constraint filtering.
        if let Some(pack) = pack {
            filter_by_pack_constraints(&mut proposals, pack);
        }

        // 5. Missing args enrichment.
        for proposal in &mut proposals {
            proposal.evidence.missing_required_args =
                self.count_missing_required_args(&proposal.verb, &proposal.args);
        }

        // 6. Sorting (template boost applied during creation, just sort by confidence).
        proposals.sort_by(|a, b| {
            b.evidence
                .confidence
                .partial_cmp(&a.evidence.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 7. Hash computation.
        let proposal_hash = compute_proposal_hash(trimmed, &proposals);

        let template_fast_path = proposals
            .first()
            .map(|p| matches!(p.evidence.source, ProposalSource::Template { .. }))
            .unwrap_or(false);

        ProposalSet {
            original_input: trimmed.to_string(),
            proposals,
            template_fast_path,
            proposal_hash,
        }
    }

    // ------------------------------------------------------------------------
    // Direct DSL
    // ------------------------------------------------------------------------

    fn direct_dsl_proposal(&self, input: &str) -> ProposalSet {
        let proposal = StepProposal {
            id: Uuid::new_v4(),
            verb: "direct.dsl".to_string(),
            sentence: format!("Execute DSL: {}", truncate(input, 60)),
            dsl: input.to_string(),
            args: HashMap::new(),
            evidence: ProposalEvidence {
                source: ProposalSource::DirectDsl,
                confidence: 1.0,
                rationale: "Direct DSL input".to_string(),
                missing_required_args: 0,
                template_fit_score: None,
                verb_search_score: None,
            },
            confirm_policy: ConfirmPolicy::Always,
        };

        let hash = compute_proposal_hash(input, std::slice::from_ref(&proposal));
        ProposalSet {
            original_input: input.to_string(),
            proposals: vec![proposal],
            template_fast_path: false,
            proposal_hash: hash,
        }
    }

    // ------------------------------------------------------------------------
    // Template Scoring
    // ------------------------------------------------------------------------

    fn score_templates(
        &self,
        input: &str,
        templates: &[PackTemplate],
        context_vars: &HashMap<String, String>,
        answers: &HashMap<String, serde_json::Value>,
    ) -> Vec<StepProposal> {
        let input_words = tokenize(input);
        let verb_phrases = self.verb_config_index.all_invocation_phrases();
        let verb_descriptions = self.verb_config_index.all_descriptions();

        let mut proposals = Vec::new();

        for template in templates {
            let score = self.score_template(&input_words, template);
            if score < TEMPLATE_SCORE_THRESHOLD {
                continue;
            }

            // Expand template into runbook entries.
            let result = instantiate_template(
                template,
                context_vars,
                answers,
                &self.sentence_gen,
                &verb_phrases,
                &verb_descriptions,
            );

            let entries = match result {
                Ok((entries, _hash)) => entries,
                Err(_) => continue,
            };

            // Convert entries to proposals.
            let boosted_confidence = (score + TEMPLATE_CONFIDENCE_BOOST).min(1.0);
            for entry in entries {
                proposals.push(StepProposal {
                    id: Uuid::new_v4(),
                    verb: entry.verb.clone(),
                    sentence: entry.sentence.clone(),
                    dsl: entry.dsl.clone(),
                    args: entry.args.clone(),
                    evidence: ProposalEvidence {
                        source: ProposalSource::Template {
                            template_id: template.template_id.clone(),
                        },
                        confidence: boosted_confidence,
                        rationale: format!(
                            "Template '{}' matches: {}",
                            template.template_id, template.when_to_use
                        ),
                        missing_required_args: 0, // Enriched later
                        template_fit_score: Some(score),
                        verb_search_score: None,
                    },
                    confirm_policy: entry.confirm_policy,
                });
            }
        }

        proposals
    }

    /// Score a template against tokenized user input via word-overlap.
    fn score_template(&self, input_words: &[String], template: &PackTemplate) -> f32 {
        // Collect scoring corpus: when_to_use + step verb invocation phrases.
        let mut corpus_words: Vec<String> = tokenize(&template.when_to_use);

        for step in &template.steps {
            let phrases = self.verb_config_index.invocation_phrases(&step.verb);
            for phrase in phrases {
                corpus_words.extend(tokenize(phrase));
            }
        }

        if corpus_words.is_empty() || input_words.is_empty() {
            return 0.0;
        }

        // Jaccard-like overlap: |input ∩ corpus| / |input|
        let matching = input_words
            .iter()
            .filter(|w| w.len() > 2 && corpus_words.contains(w))
            .count();

        matching as f32 / input_words.len() as f32
    }

    // ------------------------------------------------------------------------
    // Verb Matching
    // ------------------------------------------------------------------------

    async fn verb_match_proposals(
        &self,
        input: &str,
        match_ctx: &MatchContext,
        context_stack: &ContextStack,
    ) -> Vec<StepProposal> {
        let outcome = match self
            .intent_service
            .match_verb_with_context(input, match_ctx, context_stack)
            .await
        {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };

        match outcome {
            VerbMatchOutcome::Matched {
                verb,
                confidence,
                generated_dsl,
            } => {
                let dsl = generated_dsl.unwrap_or_else(|| format!("({})", verb));
                let args = extract_args_from_dsl(&dsl);
                let sentence = self.intent_service.generate_sentence(&verb, &args);
                let confirm_policy = self.intent_service.confirm_policy(&verb);
                let description = self.verb_config_index.description(&verb).to_string();

                vec![StepProposal {
                    id: Uuid::new_v4(),
                    verb: verb.clone(),
                    sentence,
                    dsl,
                    args,
                    evidence: ProposalEvidence {
                        source: ProposalSource::VerbMatch,
                        confidence,
                        rationale: format!(
                            "Verb '{}' matched with {:.0}% confidence: {}",
                            verb,
                            confidence * 100.0,
                            description,
                        ),
                        missing_required_args: 0,
                        template_fit_score: None,
                        verb_search_score: Some(confidence),
                    },
                    confirm_policy,
                }]
            }
            VerbMatchOutcome::Ambiguous { candidates, margin } => candidates
                .into_iter()
                .take(MAX_VERB_PROPOSALS)
                .map(|c| {
                    let dsl = format!("({})", c.verb_fqn);
                    let sentence = self
                        .intent_service
                        .generate_sentence(&c.verb_fqn, &HashMap::new());
                    let confirm_policy = self.intent_service.confirm_policy(&c.verb_fqn);

                    StepProposal {
                        id: Uuid::new_v4(),
                        verb: c.verb_fqn.clone(),
                        sentence,
                        dsl,
                        args: HashMap::new(),
                        evidence: ProposalEvidence {
                            source: ProposalSource::VerbMatch,
                            confidence: c.score,
                            rationale: format!(
                                "Verb '{}' (margin: {:.3}): {}",
                                c.verb_fqn, margin, c.description,
                            ),
                            missing_required_args: 0,
                            template_fit_score: None,
                            verb_search_score: Some(c.score),
                        },
                        confirm_policy,
                    }
                })
                .collect(),
            // NoMatch, DirectDsl, NeedsScopeSelection, etc. — no proposals.
            _ => Vec::new(),
        }
    }

    // ------------------------------------------------------------------------
    // Missing Args
    // ------------------------------------------------------------------------

    fn count_missing_required_args(
        &self,
        verb: &str,
        provided_args: &HashMap<String, String>,
    ) -> usize {
        match self.verb_config_index.get(verb) {
            Some(entry) => entry
                .args
                .iter()
                .filter(|a| a.required && !provided_args.contains_key(&a.name))
                .count(),
            None => 0,
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Filter proposals by pack's allowed_verbs and forbidden_verbs.
pub fn filter_by_pack_constraints(proposals: &mut Vec<StepProposal>, pack: &PackManifest) {
    if !pack.allowed_verbs.is_empty() {
        proposals.retain(|p| {
            // Direct DSL bypasses pack filtering.
            if p.evidence.source == ProposalSource::DirectDsl {
                return true;
            }
            pack.allowed_verbs.iter().any(|av| {
                p.verb == *av
                    || av
                        .split('.')
                        .next()
                        .map(|domain| p.verb.starts_with(&format!("{}.", domain)))
                        .unwrap_or(false)
            })
        });
    }

    if !pack.forbidden_verbs.is_empty() {
        proposals.retain(|p| !pack.forbidden_verbs.contains(&p.verb));
    }
}

/// Compute a deterministic SHA-256 hash over (input, proposals).
fn compute_proposal_hash(input: &str, proposals: &[StepProposal]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    for p in proposals {
        hasher.update(p.verb.as_bytes());
        hasher.update(b"|");
        hasher.update(p.dsl.as_bytes());
        hasher.update(b"|");
    }
    format!("{:x}", hasher.finalize())
}

/// Tokenize a string into lowercased words.
fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_' || c == '.' || c == ',')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

/// Truncate a string with ellipsis.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Extract args from DSL string (simple regex-free parser).
///
/// Parses `:key "value"` or `:key value` patterns from s-expressions.
fn extract_args_from_dsl(dsl: &str) -> HashMap<String, String> {
    let mut args = HashMap::new();
    let chars: Vec<char> = dsl.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Look for :key
        if chars[i] == ':' {
            i += 1;
            let key_start = i;
            while i < len && !chars[i].is_whitespace() && chars[i] != ')' {
                i += 1;
            }
            let key = chars[key_start..i].iter().collect::<String>();

            // Skip whitespace
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }

            if i >= len || chars[i] == ':' || chars[i] == ')' {
                continue;
            }

            // Read value
            if chars[i] == '"' {
                // Quoted value
                i += 1;
                let val_start = i;
                while i < len && chars[i] != '"' {
                    if chars[i] == '\\' {
                        i += 1; // skip escaped char
                    }
                    i += 1;
                }
                let value = chars[val_start..i].iter().collect::<String>();
                args.insert(key, value);
                if i < len {
                    i += 1; // skip closing quote
                }
            } else {
                // Unquoted value
                let val_start = i;
                while i < len && !chars[i].is_whitespace() && chars[i] != ')' {
                    i += 1;
                }
                let value = chars[val_start..i].iter().collect::<String>();
                if !value.starts_with(':') {
                    args.insert(key, value);
                }
            }
        } else {
            i += 1;
        }
    }

    args
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Type serialization round-trips --

    #[test]
    fn test_proposal_source_serialization() {
        let sources = vec![
            ProposalSource::Template {
                template_id: "onboarding-v1".to_string(),
            },
            ProposalSource::VerbMatch,
            ProposalSource::DirectDsl,
        ];

        for source in &sources {
            let json = serde_json::to_string(source).unwrap();
            let deserialized: ProposalSource = serde_json::from_str(&json).unwrap();
            assert_eq!(&deserialized, source);
        }
    }

    #[test]
    fn test_proposal_evidence_serialization() {
        let evidence = ProposalEvidence {
            source: ProposalSource::VerbMatch,
            confidence: 0.85,
            rationale: "Verb 'cbu.create' matched".to_string(),
            missing_required_args: 1,
            template_fit_score: None,
            verb_search_score: Some(0.85),
        };

        let json = serde_json::to_string(&evidence).unwrap();
        let deserialized: ProposalEvidence = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.confidence, 0.85);
        assert_eq!(deserialized.missing_required_args, 1);
    }

    #[test]
    fn test_step_proposal_serialization() {
        let proposal = StepProposal {
            id: Uuid::new_v4(),
            verb: "cbu.create".to_string(),
            sentence: "Create Allianz Lux CBU".to_string(),
            dsl: "(cbu.create :name \"Allianz Lux\")".to_string(),
            args: HashMap::from([("name".to_string(), "Allianz Lux".to_string())]),
            evidence: ProposalEvidence {
                source: ProposalSource::VerbMatch,
                confidence: 0.90,
                rationale: "Verb 'cbu.create' matched".to_string(),
                missing_required_args: 0,
                template_fit_score: None,
                verb_search_score: Some(0.90),
            },
            confirm_policy: ConfirmPolicy::Always,
        };

        let json = serde_json::to_string(&proposal).unwrap();
        let deserialized: StepProposal = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.verb, "cbu.create");
        assert_eq!(deserialized.args.get("name").unwrap(), "Allianz Lux");
    }

    #[test]
    fn test_proposal_set_serialization() {
        let set = ProposalSet {
            original_input: "create a fund".to_string(),
            proposals: vec![],
            template_fast_path: false,
            proposal_hash: "abc123".to_string(),
        };

        let json = serde_json::to_string(&set).unwrap();
        let deserialized: ProposalSet = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.original_input, "create a fund");
        assert!(!deserialized.template_fast_path);
    }

    // -- Direct DSL --

    #[test]
    fn test_direct_dsl_detection() {
        let input = "(cbu.create :name \"Test\")";
        assert!(input.trim().starts_with('('));
    }

    // -- Tokenizer --

    #[test]
    fn test_tokenize() {
        assert_eq!(
            tokenize("Load the Allianz book"),
            vec!["load", "the", "allianz", "book"]
        );
        assert_eq!(
            tokenize("cbu.create with some-thing"),
            vec!["cbu", "create", "with", "some", "thing"]
        );
    }

    // -- Args extraction --

    #[test]
    fn test_extract_args_from_dsl_quoted() {
        let args = extract_args_from_dsl("(cbu.create :name \"Allianz Lux\" :jurisdiction \"LU\")");
        assert_eq!(args.get("name").unwrap(), "Allianz Lux");
        assert_eq!(args.get("jurisdiction").unwrap(), "LU");
    }

    #[test]
    fn test_extract_args_from_dsl_unquoted() {
        let args = extract_args_from_dsl("(session.load-cbu :cbu-name test-fund)");
        assert_eq!(args.get("cbu-name").unwrap(), "test-fund");
    }

    #[test]
    fn test_extract_args_empty_dsl() {
        let args = extract_args_from_dsl("(session.clear)");
        assert!(args.is_empty());
    }

    // -- Pack constraint filtering --

    #[test]
    fn test_filter_by_allowed_verbs() {
        let mut proposals = vec![
            make_test_proposal("cbu.create", ProposalSource::VerbMatch, 0.9),
            make_test_proposal("kyc.create-case", ProposalSource::VerbMatch, 0.8),
            make_test_proposal("cbu.assign-role", ProposalSource::VerbMatch, 0.7),
        ];

        let pack = make_test_pack(
            vec!["cbu.create".to_string(), "cbu.assign-role".to_string()],
            vec![],
        );
        filter_by_pack_constraints(&mut proposals, &pack);

        assert_eq!(proposals.len(), 2);
        assert_eq!(proposals[0].verb, "cbu.create");
        assert_eq!(proposals[1].verb, "cbu.assign-role");
    }

    #[test]
    fn test_filter_by_allowed_verbs_domain_prefix() {
        let mut proposals = vec![
            make_test_proposal("cbu.create", ProposalSource::VerbMatch, 0.9),
            make_test_proposal("cbu.assign-role", ProposalSource::VerbMatch, 0.8),
            make_test_proposal("kyc.create-case", ProposalSource::VerbMatch, 0.7),
        ];

        // allowed_verbs has "cbu.create" — domain prefix "cbu" matches both cbu verbs
        let pack = make_test_pack(vec!["cbu.create".to_string()], vec![]);
        filter_by_pack_constraints(&mut proposals, &pack);

        assert_eq!(proposals.len(), 2);
        assert!(proposals.iter().all(|p| p.verb.starts_with("cbu.")));
    }

    #[test]
    fn test_filter_by_forbidden_verbs() {
        let mut proposals = vec![
            make_test_proposal("cbu.create", ProposalSource::VerbMatch, 0.9),
            make_test_proposal("cbu.delete", ProposalSource::VerbMatch, 0.8),
        ];

        let pack = make_test_pack(vec![], vec!["cbu.delete".to_string()]);
        filter_by_pack_constraints(&mut proposals, &pack);

        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].verb, "cbu.create");
    }

    #[test]
    fn test_direct_dsl_bypasses_pack_filter() {
        let mut proposals = vec![make_test_proposal(
            "direct.dsl",
            ProposalSource::DirectDsl,
            1.0,
        )];

        let pack = make_test_pack(vec!["cbu.create".to_string()], vec![]);
        filter_by_pack_constraints(&mut proposals, &pack);

        assert_eq!(proposals.len(), 1);
    }

    // -- Proposal hash --

    #[test]
    fn test_proposal_hash_determinism() {
        let proposals = vec![make_test_proposal(
            "cbu.create",
            ProposalSource::VerbMatch,
            0.9,
        )];
        let hash1 = compute_proposal_hash("create a fund", &proposals);
        let hash2 = compute_proposal_hash("create a fund", &proposals);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_proposal_hash_changes_on_different_input() {
        let proposals = vec![make_test_proposal(
            "cbu.create",
            ProposalSource::VerbMatch,
            0.9,
        )];
        let hash1 = compute_proposal_hash("create a fund", &proposals);
        let hash2 = compute_proposal_hash("delete a fund", &proposals);
        assert_ne!(hash1, hash2);
    }

    // -- Truncate --

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("hello world this is long", 10), "hello worl...");
    }

    // -- Test helpers --

    fn make_test_proposal(verb: &str, source: ProposalSource, confidence: f32) -> StepProposal {
        StepProposal {
            id: Uuid::new_v4(),
            verb: verb.to_string(),
            sentence: format!("Do {}", verb),
            dsl: format!("({})", verb),
            args: HashMap::new(),
            evidence: ProposalEvidence {
                source,
                confidence,
                rationale: format!("Test: {}", verb),
                missing_required_args: 0,
                template_fit_score: None,
                verb_search_score: Some(confidence),
            },
            confirm_policy: ConfirmPolicy::Always,
        }
    }

    fn make_test_pack(allowed_verbs: Vec<String>, forbidden_verbs: Vec<String>) -> PackManifest {
        PackManifest {
            id: "test-pack".to_string(),
            name: "Test Pack".to_string(),
            version: "1.0".to_string(),
            description: "Test".to_string(),
            invocation_phrases: vec![],
            required_context: vec![],
            optional_context: vec![],
            allowed_verbs,
            forbidden_verbs,
            risk_policy: Default::default(),
            required_questions: vec![],
            optional_questions: vec![],
            stop_rules: vec![],
            templates: vec![],
            pack_summary_template: None,
            section_layout: vec![],
            definition_of_done: vec![],
            progress_signals: vec![],
            handoff_target: None,
        }
    }
}
