//! DeterministicSage — no-LLM Sage implementation.
//!
//! Uses only pre-classification signals (plane, polarity, domain hints, action).
//! For structure-read intents with a specific domain hint, it emits a fully
//! deterministic step suitable for the Sage fast path.

use anyhow::Result;

use super::context::SageContext;
use std::collections::HashMap;

use super::outcome::{
    CoderHandoff, OutcomeAction, OutcomeIntent, OutcomeStep, SageConfidence, SageExplain,
    UtteranceHints,
};
use super::pre_classify::pre_classify;
use super::SageEngine;

/// Deterministic Sage — classifies intent without any LLM or DB calls.
///
/// Confidence is deterministic from the pre-classification. Intended for:
/// - Shadow mode (Phase 1.5) — runs alongside existing pipeline, logs comparison
/// - Unit testing — fast, deterministic
/// - Fallback when LLM Sage is unavailable
#[derive(Clone, Copy, Debug, Default)]
pub struct DeterministicSage;

#[async_trait::async_trait]
impl SageEngine for DeterministicSage {
    async fn classify(&self, utterance: &str, context: &SageContext) -> Result<OutcomeIntent> {
        let pre = pre_classify(utterance, context);

        // Derive action from first verb-like word
        let action = OutcomeAction::from_first_word(utterance);

        let domain_concept = select_domain_concept(utterance, &pre.domain_hints);
        let subject = extract_subject(utterance, &action, &domain_concept);
        let steps = build_steps(utterance, pre.sage_only, &action, &domain_concept, subject.as_ref());
        let confidence = confidence_for(&pre, &domain_concept);
        let summary = if pre.sage_only && !domain_concept.is_empty() {
            format!(
                "Describe entity schema for {domain} with fields relationships and verbs",
                domain = domain_concept
            )
        } else {
            utterance.trim()[..utterance.trim().len().min(60)].to_string()
        };

        let hints = build_hints(
            utterance,
            context,
            pre.sage_only,
            &domain_concept,
            &action,
            subject.as_ref(),
        );
        Ok(OutcomeIntent {
            summary,
            plane: pre.plane,
            polarity: pre.polarity,
            domain_concept,
            action,
            subject,
            steps,
            confidence,
            pending_clarifications: Vec::new(),
            explain: build_explain(context, utterance, confidence, pre.polarity, &hints),
            coder_handoff: build_coder_handoff(utterance, pre.polarity, &hints),
            hints,
        })
    }
}

fn build_hints(
    utterance: &str,
    context: &SageContext,
    sage_only: bool,
    domain_concept: &str,
    action: &OutcomeAction,
    subject: Option<&super::outcome::EntityRef>,
) -> UtteranceHints {
    let normalized = utterance.trim().to_ascii_lowercase();
    let mut explicit_domain_terms = Vec::new();
    if !domain_concept.is_empty() {
        explicit_domain_terms.push(domain_concept.to_string());
    }
    let explicit_action_terms = vec![action.as_str().to_string()];
    let inventory_read = matches!(action, OutcomeAction::Read)
        && (normalized.starts_with("show ")
            || normalized.starts_with("show me ")
            || normalized.starts_with("list ")
            || normalized.starts_with("what ")
            || normalized.contains(" have"));
    UtteranceHints {
        raw_preview: utterance.trim()[..utterance.trim().len().min(80)].to_string(),
        subject_phrase: subject.map(|value| value.mention.clone()),
        explicit_domain_terms,
        explicit_action_terms,
        scope_carry_forward_used: !context.last_intents.is_empty(),
        stage_focus: context.stage_focus.clone(),
        entity_kind: context.entity_kind.clone(),
        inventory_read,
        structure_read: sage_only,
        create_name_candidate: subject
            .map(|value| value.mention.clone())
            .or_else(|| extract_name_after_keyword(utterance, "called")),
    }
}

fn build_explain(
    context: &SageContext,
    utterance: &str,
    confidence: SageConfidence,
    polarity: super::IntentPolarity,
    hints: &UtteranceHints,
) -> SageExplain {
    let mode = match polarity {
        super::IntentPolarity::Read | super::IntentPolarity::Ambiguous => "read_only",
        super::IntentPolarity::Write => "confirmation_required",
    };
    let understanding = if let Some(subject) = hints.subject_phrase.as_deref() {
        format!("So you want to {} {}.", OutcomeAction::from_first_word(utterance).as_str(), subject)
    } else {
        format!("So you want to {}.", utterance.trim())
    };
    let scope_summary = context
        .dominant_entity_name
        .clone()
        .or_else(|| context.entity_kind.clone());
    SageExplain {
        understanding,
        mode: mode.to_string(),
        scope_summary,
        confidence: confidence.as_str().to_string(),
        clarifications: vec![],
    }
}

fn build_coder_handoff(
    utterance: &str,
    polarity: super::IntentPolarity,
    hints: &UtteranceHints,
) -> CoderHandoff {
    let required_outcome = if matches!(polarity, super::IntentPolarity::Write) {
        "prepare a deterministic mutation proposal".to_string()
    } else {
        "serve the current state safely".to_string()
    };
    let mut hint_terms = hints.explicit_domain_terms.clone();
    hint_terms.extend(hints.explicit_action_terms.iter().cloned());
    if let Some(subject) = hints.subject_phrase.as_ref() {
        hint_terms.push(subject.clone());
    }
    CoderHandoff {
        goal: required_outcome.clone(),
        intent_summary: utterance.trim()[..utterance.trim().len().min(80)].to_string(),
        required_outcome,
        constraints: vec![
            "respect_sem_os_surface".to_string(),
            "no_mutation_without_confirmation".to_string(),
        ],
        hint_terms,
        serve_safe: !matches!(polarity, super::IntentPolarity::Write),
        requires_confirmation: matches!(polarity, super::IntentPolarity::Write),
    }
}

fn build_steps(
    utterance: &str,
    sage_only: bool,
    action: &OutcomeAction,
    domain_concept: &str,
    subject: Option<&super::outcome::EntityRef>,
) -> Vec<OutcomeStep> {
    let mut params = HashMap::new();
    let notes = if sage_only && !domain_concept.is_empty() {
        params.insert("entity-type".to_string(), domain_concept.to_string());
        Some("deterministic_structure_read".to_string())
    } else {
        populate_instance_params(&mut params, utterance, action, subject);
        None
    };

    if params.is_empty() && notes.is_none() {
        return Vec::new();
    }

    vec![OutcomeStep {
        action: action.clone(),
        target: domain_concept.to_string(),
        params,
        notes,
    }]
}

fn populate_instance_params(
    params: &mut HashMap<String, String>,
    utterance: &str,
    action: &OutcomeAction,
    subject: Option<&super::outcome::EntityRef>,
) {
    if matches!(action, OutcomeAction::Create) {
        if let Some(subject) = subject.map(|subject| subject.mention.trim()).filter(|s| !s.is_empty()) {
            params.insert("name".to_string(), subject.to_string());
        } else if let Some(name) = extract_name_after_keyword(utterance, "called") {
            params.insert("name".to_string(), name);
        }
    }
}

fn extract_subject(
    utterance: &str,
    action: &OutcomeAction,
    domain_concept: &str,
) -> Option<super::outcome::EntityRef> {
    if matches!(action, OutcomeAction::Create) {
        if let Some(mention) = extract_name_after_keyword(utterance, "for") {
            return Some(super::outcome::EntityRef {
                mention,
                kind_hint: (!domain_concept.is_empty()).then(|| domain_concept.to_string()),
                uuid: None,
            });
        }
        if let Some(mention) = extract_name_after_keyword(utterance, "called") {
            return Some(super::outcome::EntityRef {
                mention,
                kind_hint: (!domain_concept.is_empty()).then(|| domain_concept.to_string()),
                uuid: None,
            });
        }
    }

    None
}

fn extract_name_after_keyword(utterance: &str, keyword: &str) -> Option<String> {
    let normalized = utterance.trim();
    let lower = normalized.to_ascii_lowercase();
    let needle = format!(" {keyword} ");
    let start = lower.find(&needle).map(|idx| idx + needle.len())?;
    let candidate = normalized[start..].trim().trim_matches(|c| c == '"' || c == '\'');
    if candidate.is_empty() {
        None
    } else {
        Some(candidate.to_string())
    }
}

fn select_domain_concept(utterance: &str, domain_hints: &[String]) -> String {
    let normalized = utterance.trim().to_ascii_lowercase();
    if normalized.contains("teach the system") || normalized.contains("research mode") {
        return "agent".to_string();
    }
    if normalized.contains(" cbu")
        || normalized.starts_with("cbu ")
        || normalized.contains("client business unit")
    {
        return "cbu".to_string();
    }
    if normalized.contains("beneficial owner")
        || normalized.contains("ownership structure")
        || normalized.contains("who controls")
    {
        return "ubo".to_string();
    }
    if normalized.contains("icav")
        || normalized.contains("sicav")
        || normalized.contains("oeic")
        || normalized.contains("40-act")
        || normalized.contains("fund structure")
    {
        return "struct".to_string();
    }
    if normalized.contains("kyc case")
        || normalized.contains("open a case")
        || normalized.contains("new case")
    {
        return "case".to_string();
    }
    if normalized.contains("collect documents")
        || normalized.contains("request identity documents")
        || normalized.contains("full kyc")
    {
        return "kyc".to_string();
    }
    if normalized.contains(" document") || normalized.starts_with("document ") {
        return "document".to_string();
    }
    if normalized.contains(" deal") || normalized.starts_with("deal ") {
        return "deal".to_string();
    }

    const GENERIC_HINTS: &[&str] = &[
        "schema",
        "struct",
        "structure",
        "entity",
        "field",
        "fields",
        "relationship",
        "relationships",
        "verb",
        "verbs",
        "attribute",
        "attributes",
        "record",
        "records",
    ];

    domain_hints
        .iter()
        .find(|hint| !GENERIC_HINTS.contains(&hint.as_str()))
        .cloned()
        .or_else(|| domain_hints.first().cloned())
        .unwrap_or_default()
}

fn confidence_for(
    pre: &super::pre_classify::SagePreClassification,
    domain_concept: &str,
) -> SageConfidence {
    let margin = pre.domain_score - pre.runner_up_domain_score;

    if pre.sage_only && !domain_concept.is_empty() {
        SageConfidence::High
    } else if !domain_concept.is_empty() && pre.domain_score >= 10 && margin >= 4 {
        SageConfidence::High
    } else if !domain_concept.is_empty() && pre.domain_score >= 6 && margin >= 2 {
        SageConfidence::Medium
    } else {
        SageConfidence::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sage::{CoderEngine, IntentPolarity, ObservationPlane, SageConfidence};

    fn ctx_with_focus(stage_focus: &str) -> SageContext {
        SageContext {
            session_id: None,
            stage_focus: Some(stage_focus.to_string()),
            goals: vec![],
            entity_kind: None,
            dominant_entity_name: None,
            last_intents: vec![],
        }
    }

    fn empty_ctx() -> SageContext {
        SageContext::default()
    }

    #[tokio::test]
    async fn test_deterministic_sage_returns_low_confidence_without_domain_hint() {
        let sage = DeterministicSage;
        let ctx = empty_ctx();
        let result = sage.classify("show me all things", &ctx).await.unwrap();
        assert_eq!(result.confidence, SageConfidence::Low);
    }

    #[tokio::test]
    async fn test_deterministic_sage_structure_plane() {
        let sage = DeterministicSage;
        let ctx = ctx_with_focus("semos-data-management");
        let result = sage.classify("list entity types", &ctx).await.unwrap();
        assert_eq!(result.plane, ObservationPlane::Structure);
        assert_eq!(result.polarity, IntentPolarity::Read);
    }

    #[tokio::test]
    async fn test_deterministic_sage_registry_plane() {
        let sage = DeterministicSage;
        let ctx = ctx_with_focus("semos-stewardship");
        let result = sage
            .classify("show pending changesets", &ctx)
            .await
            .unwrap();
        assert_eq!(result.plane, ObservationPlane::Registry);
    }

    #[tokio::test]
    async fn test_deterministic_sage_instance_plane_default() {
        let sage = DeterministicSage;
        let ctx = empty_ctx();
        let result = sage.classify("create a new fund", &ctx).await.unwrap();
        assert_eq!(result.plane, ObservationPlane::Instance);
    }

    #[tokio::test]
    async fn test_deterministic_sage_domain_hint_extracted() {
        let sage = DeterministicSage;
        let ctx = empty_ctx();
        let result = sage
            .classify("describe the deal schema", &ctx)
            .await
            .unwrap();
        // domain_concept should be first domain hint — "deal" or "schema"
        assert!(!result.domain_concept.is_empty());
    }

    #[tokio::test]
    async fn test_deterministic_sage_empty_utterance() {
        let sage = DeterministicSage;
        let ctx = empty_ctx();
        // Should not panic on empty
        let result = sage.classify("", &ctx).await.unwrap();
        assert_eq!(result.confidence, SageConfidence::Low);
    }

    #[tokio::test]
    async fn test_deterministic_sage_fast_path_shape_for_structure_read() {
        let sage = DeterministicSage;
        let ctx = ctx_with_focus("semos-data-management");
        let result = sage.classify("show me documents", &ctx).await.unwrap();

        assert_eq!(result.plane, ObservationPlane::Structure);
        assert_eq!(result.polarity, IntentPolarity::Read);
        assert_eq!(result.domain_concept, "document");
        assert_eq!(result.confidence, SageConfidence::High);
        assert_eq!(
            result
                .steps
                .first()
                .and_then(|step| step.params.get("entity-type"))
                .map(String::as_str),
            Some("document")
        );
    }

    #[tokio::test]
    async fn test_deterministic_sage_coder_resolves_schema_describe() {
        let sage = DeterministicSage;
        let ctx = ctx_with_focus("semos-data-management");
        let outcome = sage.classify("show me documents", &ctx).await.unwrap();
        let coder = CoderEngine::load().unwrap();
        let result = coder.resolve(&outcome).unwrap();

        assert_eq!(result.verb_fqn, "schema.entity.describe");
        assert!(result.missing_args.is_empty());
        assert_eq!(
            result.dsl,
            "(schema.entity.describe :entity-type \"document\")"
        );
    }

    #[tokio::test]
    async fn test_deterministic_sage_coder_resolves_plural_cbus_to_list() {
        let sage = DeterministicSage;
        let ctx = empty_ctx();
        let outcome = sage.classify("show me the cbus", &ctx).await.unwrap();
        let coder = CoderEngine::load().unwrap();
        let result = coder.resolve(&outcome).unwrap();

        assert_eq!(outcome.domain_concept, "cbu");
        assert_eq!(result.verb_fqn, "cbu.list");
    }

    #[tokio::test]
    async fn test_deterministic_sage_preserves_inventory_summary_for_deal_list() {
        let sage = DeterministicSage;
        let ctx = empty_ctx();
        let outcome = sage
            .classify("what deals does Allianz have?", &ctx)
            .await
            .unwrap();
        let coder = CoderEngine::load().unwrap();
        let result = coder.resolve(&outcome).unwrap();

        assert_eq!(outcome.summary, "what deals does Allianz have?");
        assert_eq!(result.verb_fqn, "deal.list");
    }

    #[tokio::test]
    async fn test_deterministic_sage_extracts_cbu_name_for_create() {
        let sage = DeterministicSage;
        let ctx = empty_ctx();
        let outcome = sage
            .classify("create a new CBU for Allianz UK Fund", &ctx)
            .await
            .unwrap();
        let coder = CoderEngine::load().unwrap();
        let result = coder.resolve(&outcome).unwrap();

        assert_eq!(
            outcome.subject.as_ref().map(|subject| subject.mention.as_str()),
            Some("Allianz UK Fund")
        );
        assert_eq!(
            outcome
                .steps
                .first()
                .and_then(|step| step.params.get("name"))
                .map(String::as_str),
            Some("Allianz UK Fund")
        );
        assert_eq!(result.verb_fqn, "cbu.create", "{result:?}");
        assert!(result.missing_args.is_empty(), "{result:?}");
    }

    #[tokio::test]
    async fn test_deterministic_sage_summary_truncated() {
        let sage = DeterministicSage;
        let ctx = empty_ctx();
        let long = "show me all the funds that are in luxembourg and have been registered since 2020 and have more than 100 sub-funds";
        let result = sage.classify(long, &ctx).await.unwrap();
        assert!(result.summary.len() <= 60);
    }

    #[tokio::test]
    async fn test_case_phrase_gets_high_confidence() {
        let sage = DeterministicSage;
        let ctx = empty_ctx();
        let result = sage
            .classify("open a new case and collect the KYC documents", &ctx)
            .await
            .unwrap();
        assert_eq!(result.domain_concept, "case");
        assert_eq!(result.confidence, SageConfidence::High);
    }

    #[tokio::test]
    async fn test_struct_phrase_beats_fund() {
        let sage = DeterministicSage;
        let ctx = empty_ctx();
        let result = sage
            .classify("Set up an Irish ICAV fund", &ctx)
            .await
            .unwrap();
        assert_eq!(result.domain_concept, "struct");
    }
}
