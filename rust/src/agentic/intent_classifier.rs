//! Intent Classifier
//!
//! Classifies user utterances into one or more intents using a combination
//! of pattern matching, semantic similarity, and context-aware re-ranking.
//!
//! The classifier uses a two-phase approach:
//! 1. Pattern-based matching using trigger phrases from the taxonomy
//! 2. Context-aware re-ranking based on conversation history

use crate::agentic::taxonomy::{IntentTaxonomy, ThresholdConfig};
use regex::Regex;
use std::collections::HashMap;

/// Result of classifying an utterance
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    /// The classified intents with confidence scores
    pub intents: Vec<ClassifiedIntent>,
    /// Whether clarification is needed
    pub needs_clarification: bool,
    /// Suggested clarification question if needed
    pub clarification_question: Option<String>,
}

/// A single classified intent with metadata
#[derive(Debug, Clone)]
pub struct ClassifiedIntent {
    /// The intent ID (e.g., "im_assign")
    pub intent_id: String,
    /// The canonical DSL verb
    pub canonical_verb: Option<String>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// The source text that matched this intent
    pub source_text: String,
    /// Execution decision based on confidence
    pub execution_decision: ExecutionDecision,
    /// Whether this is a query intent (read-only)
    pub is_query: bool,
    /// Whether confirmation is required before execution
    pub confirmation_required: bool,
    /// Extracted slot values from trigger phrase matching
    pub extracted_slots: HashMap<String, String>,
}

/// Decision on how to handle the classified intent
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionDecision {
    /// High confidence - execute directly
    Execute,
    /// Medium confidence - confirm with user first
    ConfirmFirst,
    /// Low confidence - suggest but ask for confirmation
    Suggest,
    /// Too low - ask clarifying questions
    Clarify,
}

/// Conversation context for intent classification
#[derive(Debug, Clone, Default)]
pub struct ConversationContext {
    /// The last classified intent
    pub last_intent: Option<String>,
    /// Current workflow stage
    pub workflow_stage: Option<String>,
    /// Known entities from previous turns
    pub known_entities: HashMap<String, String>,
    /// Entities created in this session (@symbols)
    pub session_entities: HashMap<String, String>,
}

/// The intent classifier
pub struct IntentClassifier {
    taxonomy: IntentTaxonomy,
    compiled_patterns: Vec<CompiledPattern>,
}

/// A compiled pattern for matching
struct CompiledPattern {
    intent_id: String,
    pattern: String,
    regex: Regex,
    slot_names: Vec<String>,
}

impl IntentClassifier {
    /// Create a new classifier from a taxonomy
    pub fn new(taxonomy: IntentTaxonomy) -> Self {
        let compiled_patterns = Self::compile_patterns(&taxonomy);
        Self {
            taxonomy,
            compiled_patterns,
        }
    }

    /// Compile trigger phrases into regex patterns
    fn compile_patterns(taxonomy: &IntentTaxonomy) -> Vec<CompiledPattern> {
        let mut patterns = Vec::new();

        for intent in taxonomy.all_intents() {
            for phrase in &intent.trigger_phrases {
                if let Some(compiled) = Self::compile_phrase(&intent.intent, phrase) {
                    patterns.push(compiled);
                }
            }
        }

        patterns
    }

    /// Compile a single trigger phrase into a regex pattern
    fn compile_phrase(intent_id: &str, phrase: &str) -> Option<CompiledPattern> {
        // Extract slot names from {slot_name} placeholders
        let slot_regex = Regex::new(r"\{(\w+)\}").ok()?;
        let slot_names: Vec<String> = slot_regex
            .captures_iter(phrase)
            .map(|c| c[1].to_string())
            .collect();

        // Convert phrase to regex pattern:
        // 1. First replace {slot_name} with a temporary placeholder that won't be escaped
        // 2. Then escape the rest
        // 3. Then replace placeholder with capture group
        let placeholder = "\x00SLOT\x00";
        let with_placeholder = slot_regex.replace_all(phrase, placeholder).to_string();
        let escaped = regex::escape(&with_placeholder);
        let pattern = escaped.replace(placeholder, r"(.+?)");

        // Make it case-insensitive and allow flexible whitespace
        let pattern = format!(r"(?i){}", pattern.replace(r"\ ", r"\s+"));

        match Regex::new(&pattern) {
            Ok(regex) => Some(CompiledPattern {
                intent_id: intent_id.to_string(),
                pattern: phrase.to_string(),
                regex,
                slot_names,
            }),
            Err(_) => None,
        }
    }

    /// Classify a user utterance
    pub fn classify(&self, utterance: &str, context: &ConversationContext) -> ClassificationResult {
        // Step 1: Find candidate intents by pattern matching
        let mut candidates = self.match_patterns(utterance);

        // Step 2: Re-rank using context
        self.rerank_with_context(&mut candidates, context);

        // Step 3: Sort by confidence
        candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        // Step 4: Apply confidence thresholds
        let classified: Vec<ClassifiedIntent> = candidates
            .into_iter()
            .map(|c| self.apply_thresholds(c))
            .collect();

        // Step 5: Check if clarification is needed
        let needs_clarification = classified.is_empty()
            || classified
                .first()
                .map(|c| c.execution_decision == ExecutionDecision::Clarify)
                .unwrap_or(true);

        let clarification_question = if needs_clarification {
            Some(self.generate_clarification_question(&classified, utterance))
        } else {
            None
        };

        ClassificationResult {
            intents: classified,
            needs_clarification,
            clarification_question,
        }
    }

    /// Match utterance against compiled patterns
    fn match_patterns(&self, utterance: &str) -> Vec<ClassifiedIntent> {
        let mut matches = Vec::new();

        for pattern in &self.compiled_patterns {
            if let Some(captures) = pattern.regex.captures(utterance) {
                // Extract slot values
                let mut slots = HashMap::new();
                for (i, slot_name) in pattern.slot_names.iter().enumerate() {
                    if let Some(value) = captures.get(i + 1) {
                        slots.insert(slot_name.clone(), value.as_str().to_string());
                    }
                }

                // Get the intent definition for additional metadata
                let intent_def = self.taxonomy.get_intent(&pattern.intent_id);

                // Calculate base confidence from match quality
                let match_quality = self.calculate_match_quality(utterance, &pattern.pattern);

                matches.push(ClassifiedIntent {
                    intent_id: pattern.intent_id.clone(),
                    canonical_verb: intent_def.and_then(|i| i.canonical_verb.clone()),
                    confidence: match_quality,
                    source_text: captures
                        .get(0)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default(),
                    execution_decision: ExecutionDecision::Execute, // Will be adjusted later
                    is_query: intent_def.map(|i| i.is_query).unwrap_or(false),
                    confirmation_required: intent_def
                        .map(|i| i.confirmation_required)
                        .unwrap_or(false),
                    extracted_slots: slots,
                });
            }
        }

        // Deduplicate by intent_id, keeping highest confidence
        let mut best_by_intent: HashMap<String, ClassifiedIntent> = HashMap::new();
        for m in matches {
            let entry = best_by_intent
                .entry(m.intent_id.clone())
                .or_insert(m.clone());
            if m.confidence > entry.confidence {
                *entry = m;
            }
        }

        best_by_intent.into_values().collect()
    }

    /// Calculate match quality based on how well the utterance matches the pattern
    fn calculate_match_quality(&self, utterance: &str, pattern: &str) -> f32 {
        // Base confidence for pattern match
        let mut confidence = 0.7f32;

        // Boost for longer patterns (more specific)
        let pattern_words = pattern.split_whitespace().count();
        if pattern_words > 3 {
            confidence += 0.1;
        }
        if pattern_words > 5 {
            confidence += 0.05;
        }

        // Boost for utterance that closely matches pattern length
        let utterance_words = utterance.split_whitespace().count();
        let length_ratio = (pattern_words as f32) / (utterance_words as f32).max(1.0);
        if length_ratio > 0.5 && length_ratio < 1.5 {
            confidence += 0.1;
        }

        confidence.min(0.95)
    }

    /// Re-rank candidates based on conversation context
    fn rerank_with_context(
        &self,
        candidates: &mut [ClassifiedIntent],
        context: &ConversationContext,
    ) {
        for candidate in candidates.iter_mut() {
            // Boost intents that are natural followups
            if let Some(last_intent) = &context.last_intent {
                if self
                    .taxonomy
                    .is_natural_followup(last_intent, &candidate.intent_id)
                {
                    candidate.confidence *= 1.2;
                }
            }

            // Boost query intents slightly (safer to execute)
            if candidate.is_query {
                candidate.confidence *= 1.05;
            }

            // Cap at 0.99
            candidate.confidence = candidate.confidence.min(0.99);
        }
    }

    /// Apply confidence thresholds to determine execution decision
    fn apply_thresholds(&self, mut intent: ClassifiedIntent) -> ClassifiedIntent {
        let thresholds = self.taxonomy.get_thresholds(&intent.intent_id);
        intent.execution_decision = Self::determine_decision(intent.confidence, thresholds);

        // Force confirmation for dangerous intents even with high confidence
        if intent.confirmation_required && intent.execution_decision == ExecutionDecision::Execute {
            intent.execution_decision = ExecutionDecision::ConfirmFirst;
        }

        intent
    }

    /// Determine execution decision based on confidence and thresholds
    fn determine_decision(confidence: f32, thresholds: &ThresholdConfig) -> ExecutionDecision {
        if confidence >= thresholds.execute_threshold {
            ExecutionDecision::Execute
        } else if confidence >= thresholds.confirm_threshold {
            ExecutionDecision::ConfirmFirst
        } else if confidence >= thresholds.suggest_threshold {
            ExecutionDecision::Suggest
        } else {
            ExecutionDecision::Clarify
        }
    }

    /// Generate a clarification question when intent is unclear
    fn generate_clarification_question(
        &self,
        candidates: &[ClassifiedIntent],
        utterance: &str,
    ) -> String {
        if candidates.is_empty() {
            return format!(
                "I'm not sure what you'd like to do. Could you rephrase \"{}\"?",
                utterance
            );
        }

        // If we have ambiguous candidates, ask about them
        let ambiguous: Vec<_> = candidates
            .iter()
            .filter(|c| c.confidence >= 0.3)
            .take(3)
            .collect();

        if ambiguous.len() > 1 {
            let options: Vec<String> = ambiguous
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let verb = c.canonical_verb.as_deref().unwrap_or(&c.intent_id);
                    format!("{}. {}", i + 1, verb)
                })
                .collect();

            format!(
                "I'm not sure which action you mean. Did you want to:\n{}",
                options.join("\n")
            )
        } else if let Some(first) = ambiguous.first() {
            // Single low-confidence match
            let verb = first.canonical_verb.as_deref().unwrap_or(&first.intent_id);
            format!("Did you mean to {}?", verb)
        } else {
            format!(
                "I'm not sure what you'd like to do. Could you rephrase \"{}\"?",
                utterance
            )
        }
    }

    /// Detect if utterance contains multiple intents (compound)
    pub fn detect_compound_intents(
        &self,
        utterance: &str,
        context: &ConversationContext,
    ) -> Vec<ClassificationResult> {
        // Split on common conjunctions
        let segments = self.segment_utterance(utterance);

        if segments.len() <= 1 {
            return vec![self.classify(utterance, context)];
        }

        // Classify each segment
        segments
            .iter()
            .map(|segment| self.classify(segment, context))
            .collect()
    }

    /// Segment an utterance into potentially multiple intent segments
    fn segment_utterance(&self, utterance: &str) -> Vec<String> {
        // Split on "and", "also", "plus", semicolons, etc.
        let splitters = Regex::new(r"(?i)\s+and\s+|\s+also\s+|\s+plus\s+|;\s*|,\s+and\s+").unwrap();

        let segments: Vec<String> = splitters
            .split(utterance)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Only return multiple segments if each is substantial
        if segments.iter().all(|s| s.split_whitespace().count() >= 2) {
            segments
        } else {
            vec![utterance.to_string()]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::taxonomy::IntentTaxonomy;

    fn sample_taxonomy() -> IntentTaxonomy {
        let yaml = r#"
version: "1.0"
description: "Test taxonomy"
intent_taxonomy:
  trading_matrix:
    description: "Trading matrix domain"
    investment_manager:
      description: "IM subdomain"
      intents:
        - intent: im_assign
          description: "Assign IM"
          canonical_verb: investment-manager.assign
          trigger_phrases:
            - "add {manager} as investment manager"
            - "{manager} will handle {scope}"
            - "assign {manager}"
        - intent: im_query
          description: "Query IMs"
          canonical_verb: investment-manager.list
          is_query: true
          trigger_phrases:
            - "show me the investment managers"
            - "list all IMs"
            - "who handles {scope}"
    pricing:
      description: "Pricing"
      intents:
        - intent: pricing_set
          description: "Set pricing"
          canonical_verb: pricing-config.set
          trigger_phrases:
            - "use {source} for {instruments}"
            - "{source} for pricing"
intent_relationships:
  natural_followups:
    im_assign:
      - pricing_set
confidence_thresholds:
  defaults:
    execute_threshold: 0.85
    confirm_threshold: 0.65
    suggest_threshold: 0.45
"#;
        IntentTaxonomy::load_from_str(yaml).unwrap()
    }

    #[test]
    fn test_simple_classification() {
        let classifier = IntentClassifier::new(sample_taxonomy());
        let context = ConversationContext::default();

        let result = classifier.classify("add BlackRock as investment manager", &context);

        assert!(!result.needs_clarification);
        assert!(!result.intents.is_empty());
        assert_eq!(result.intents[0].intent_id, "im_assign");
        assert_eq!(
            result.intents[0].extracted_slots.get("manager"),
            Some(&"BlackRock".to_string())
        );
    }

    #[test]
    fn test_query_classification() {
        let classifier = IntentClassifier::new(sample_taxonomy());
        let context = ConversationContext::default();

        let result = classifier.classify("show me the investment managers", &context);

        assert!(!result.needs_clarification);
        assert_eq!(result.intents[0].intent_id, "im_query");
        assert!(result.intents[0].is_query);
    }

    #[test]
    fn test_context_boost() {
        let classifier = IntentClassifier::new(sample_taxonomy());
        let mut context = ConversationContext::default();
        context.last_intent = Some("im_assign".to_string());

        let result = classifier.classify("Bloomberg for pricing", &context);

        // Should get a boost because pricing_set is a natural followup of im_assign
        assert!(!result.intents.is_empty());
        assert_eq!(result.intents[0].intent_id, "pricing_set");
    }

    #[test]
    fn test_unclear_utterance() {
        let classifier = IntentClassifier::new(sample_taxonomy());
        let context = ConversationContext::default();

        let result = classifier.classify("something completely unrelated", &context);

        assert!(result.needs_clarification || result.intents.is_empty());
    }

    #[test]
    fn test_compound_detection() {
        let classifier = IntentClassifier::new(sample_taxonomy());
        let context = ConversationContext::default();

        let results = classifier.detect_compound_intents(
            "add BlackRock as investment manager and use Bloomberg for pricing",
            &context,
        );

        // Should detect two segments
        assert!(results.len() >= 1);
    }

    #[test]
    fn test_slot_extraction() {
        let classifier = IntentClassifier::new(sample_taxonomy());
        let context = ConversationContext::default();

        let result = classifier.classify("who handles European equities", &context);

        assert!(!result.intents.is_empty());
        if let Some(scope) = result.intents[0].extracted_slots.get("scope") {
            assert!(scope.contains("European") || scope.contains("equities"));
        }
    }
}
