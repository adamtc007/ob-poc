//! DeterministicSage — no-LLM Sage implementation.
//!
//! Uses only pre-classification signals (plane, polarity, domain hints, action).
//! For structure-read intents with a specific domain hint, it emits a fully
//! deterministic step suitable for the Sage fast path.

use anyhow::Result;

use super::context::SageContext;
use std::collections::HashMap;

use super::outcome::{OutcomeAction, OutcomeIntent, OutcomeStep, SageConfidence};
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
        })
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
    if normalized.contains(" cbu")
        || normalized.starts_with("cbu ")
        || normalized.contains("client business unit")
    {
        return "cbu".to_string();
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
    if pre.sage_only && !domain_concept.is_empty() {
        SageConfidence::High
    } else if !domain_concept.is_empty() {
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
}
