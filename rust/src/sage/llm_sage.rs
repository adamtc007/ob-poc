//! LlmSage — LLM-backed Sage implementation with deterministic fallback.
//!
//! The deterministic pre-classifier remains the source of truth for plane and
//! polarity. The LLM is constrained to choose a domain, action family, and
//! extract concrete parameters from the utterance.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use anyhow::{Context, Result};
use ob_agentic::llm_client::{LlmClient, ToolDefinition};
use serde::{Deserialize, Serialize};

use super::context::SageContext;
use super::deterministic::DeterministicSage;
use super::outcome::{
    CoderHandoff, OutcomeAction, OutcomeIntent, OutcomeStep, SageConfidence, SageExplain,
    UtteranceHints,
};
use super::pre_classify::{pre_classify, SagePreClassification};
use super::{IntentPolarity, SageEngine};

const READ_DOMAINS: &[&str] = &[
    "cbu",
    "entity",
    "fund",
    "deal",
    "document",
    "screening",
    "ubo",
    "ownership",
    "control",
    "gleif",
    "bods",
    "client-group",
    "session",
    "view",
    "billing",
    "sla",
    "team",
    "registry",
    "schema",
    "agent",
];

const WRITE_DOMAINS: &[&str] = &[
    "cbu",
    "entity",
    "fund",
    "deal",
    "document",
    "screening",
    "ubo",
    "ownership",
    "control",
    "gleif",
    "bods",
    "client-group",
    "session",
    "view",
    "billing",
    "sla",
    "team",
    "registry",
    "schema",
    "agent",
    "trading-profile",
    "capital",
    "movement",
    "investor",
    "contract",
    "lifecycle",
    "service-resource",
    "requirement",
    "settlement-chain",
    "kyc-case",
];

const READ_ACTIONS: &[&str] = &[
    "investigate",
    "report",
    "trace",
    "assess-readonly",
];

const WRITE_ACTIONS: &[&str] = &[
    "create",
    "modify",
    "link",
    "remove",
    "transfer",
    "assess-mutating",
    "configure",
    "verify",
];

const PARAM_HINTS: &[(&str, &str)] = &[
    ("cbu", "name, jurisdiction (ISO 2-letter), fund-entity-id, client-type, description"),
    ("entity", "name, entity-type (limited-company|proper-person|trust-discretionary|partnership-limited), jurisdiction"),
    ("fund", "name, fund-type (umbrella|subfund|share-class|standalone|master|feeder), parent-fund, jurisdiction"),
    ("deal", "deal-id, status, client"),
    ("document", "entity-name, document-type, file-reference"),
    ("screening", "entity-name, screening-type (sanctions|pep|adverse-media)"),
    ("ubo", "entity-name, ownership-percentage, relationship-type (ownership|control|trust-role)"),
    ("ownership", "entity-name, issuer, percentage"),
    ("gleif", "lei, entity-name, client-group"),
    ("client-group", "group-name, entity-name, role"),
    ("session", "target (cbu-name|client-name|deal-id), jurisdiction"),
    ("view", "target, level (universe|galaxy|system|planet)"),
];

/// LLM-backed Sage engine.
///
/// Falls back to `DeterministicSage` when the provider call or JSON parsing fails.
#[derive(Clone)]
pub struct LlmSage {
    client: Arc<dyn LlmClient>,
    fallback: DeterministicSage,
}

impl LlmSage {
    /// Create a new LLM-backed Sage.
    ///
    /// # Examples
    /// ```ignore
    /// use std::sync::Arc;
    /// use ob_agentic::openai_client::OpenAiClient;
    /// use ob_poc::sage::LlmSage;
    ///
    /// let client = Arc::new(OpenAiClient::from_env()?);
    /// let sage = LlmSage::new(client);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new(client: Arc<dyn LlmClient>) -> Self {
        Self {
            client,
            fallback: DeterministicSage,
        }
    }

    fn system_prompt() -> &'static str {
        r#"You are an outcome classifier for a custody banking onboarding platform.

Given a user utterance and context, identify what the user wants to achieve.
You are NOT selecting a function or verb. You are identifying the business outcome.

RULES:
1. Respond with only valid JSON tool arguments.
2. For "domain": pick from the provided domain list.
3. For "action": pick from the provided action list.
4. For "params": extract concrete values the user mentioned.
5. For "confidence": choose high, medium, or low.
6. Keep "summary" to one sentence describing the outcome in business terms.
7. Respect the provided observation plane and polarity as hard constraints."#
    }

    fn build_user_prompt(
        &self,
        utterance: &str,
        context: &SageContext,
        pre: &SagePreClassification,
    ) -> String {
        let domains = domain_list_for(pre.polarity).join(", ");
        let actions = action_list_for(pre.polarity).join(", ");
        let param_hints = render_param_hints(domain_list_for(pre.polarity));
        let workflow = context.stage_focus.as_deref().unwrap_or("general");
        let current_entity = context
            .dominant_entity_name
            .as_deref()
            .unwrap_or("none");
        let entity_type = context.entity_kind.as_deref().unwrap_or("unknown");
        let recent_actions = if context.last_intents.is_empty() {
            "none".to_string()
        } else {
            context
                .last_intents
                .iter()
                .map(|recent| format!("{}:{}:{}", recent.plane, recent.domain_concept, recent.action))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let domain_hints = if pre.domain_hints.is_empty() {
            "none".to_string()
        } else {
            pre.domain_hints.join(", ")
        };

        format!(
            "UTTERANCE: \"{utterance}\"\n\n\
CONTEXT:\n  Workflow: {workflow}\n  Current entity: {current_entity}\n  Entity type: {entity_type}\n  Recent actions: {recent_actions}\n\n\
PRE-CLASSIFICATION (already determined):\n  Observation plane: {plane}\n  Intent polarity: {polarity}\n  Domain hints: {domain_hints}\n\n\
DOMAIN LIST (for {polarity} operations):\n{domains}\n\n\
ACTION LIST (for {polarity} operations):\n{actions}\n\n\
PARAMETER HINTS BY DOMAIN:\n{param_hints}\n\n\
Respond with JSON fields:\n{{\n  \"summary\": \"one sentence business outcome\",\n  \"domain\": \"domain from list above\",\n  \"action\": \"action from list above\",\n  \"params\": {{\"param_name\": \"extracted_value\"}},\n  \"confidence\": \"high|medium|low\"\n}}",
            plane = pre.plane.as_str(),
            polarity = pre.polarity.as_str(),
        )
    }

    fn tool_definition() -> ToolDefinition {
        ToolDefinition {
            name: "classify_sage_outcome".to_string(),
            description: "Classify business outcome, domain, action family, extracted params, and confidence".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string" },
                    "domain": { "type": "string" },
                    "action": { "type": "string" },
                    "params": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    },
                    "confidence": {
                        "type": "string",
                        "enum": ["high", "medium", "low"]
                    }
                },
                "required": ["summary", "domain", "action", "params", "confidence"]
            }),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct LlmOutcomePayload {
    summary: String,
    domain: String,
    action: String,
    #[serde(default)]
    params: BTreeMap<String, String>,
    confidence: String,
}

fn domain_list_for(polarity: IntentPolarity) -> &'static [&'static str] {
    match polarity {
        IntentPolarity::Read => READ_DOMAINS,
        IntentPolarity::Write => WRITE_DOMAINS,
        IntentPolarity::Ambiguous => WRITE_DOMAINS,
    }
}

fn action_list_for(polarity: IntentPolarity) -> &'static [&'static str] {
    match polarity {
        IntentPolarity::Read => READ_ACTIONS,
        IntentPolarity::Write => WRITE_ACTIONS,
        IntentPolarity::Ambiguous => WRITE_ACTIONS,
    }
}

fn render_param_hints(domains: &[&str]) -> String {
    PARAM_HINTS
        .iter()
        .filter(|(domain, _)| domains.contains(domain))
        .map(|(domain, hints)| format!("{domain}: {hints}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_outcome_action(raw: &str, polarity: IntentPolarity) -> OutcomeAction {
    match raw.trim().to_ascii_lowercase().as_str() {
        "investigate" | "report" | "trace" | "assess-readonly" => OutcomeAction::Read,
        "create" => OutcomeAction::Create,
        "modify" | "configure" => OutcomeAction::Update,
        "link" => OutcomeAction::Assign,
        "remove" => OutcomeAction::Delete,
        "transfer" => OutcomeAction::Import,
        "assess-mutating" => OutcomeAction::Compute,
        "verify" => OutcomeAction::Publish,
        _ if polarity == IntentPolarity::Read => OutcomeAction::Read,
        other => OutcomeAction::Other(other.to_string()),
    }
}

fn parse_confidence(raw: &str) -> SageConfidence {
    match raw.trim().to_ascii_lowercase().as_str() {
        "high" => SageConfidence::High,
        "medium" => SageConfidence::Medium,
        _ => SageConfidence::Low,
    }
}

fn extract_json(response: &str) -> &str {
    let trimmed = response.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        return stripped
            .trim()
            .strip_suffix("```")
            .map(str::trim)
            .unwrap_or(trimmed);
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        return stripped
            .trim()
            .strip_suffix("```")
            .map(str::trim)
            .unwrap_or(trimmed);
    }
    trimmed
}

fn parse_sage_response(
    utterance: &str,
    response: &str,
    context: &SageContext,
    pre: &SagePreClassification,
) -> Result<OutcomeIntent> {
    let payload: LlmOutcomePayload =
        serde_json::from_str(extract_json(response)).context("failed to parse Sage response")?;

    let domain = payload.domain.trim().to_string();
    let action = parse_outcome_action(&payload.action, pre.polarity);
    let summary = payload.summary.trim().to_string();
    let confidence = apply_asymmetric_risk(parse_confidence(&payload.confidence), pre.polarity);
    let params = payload
        .params
        .into_iter()
        .filter_map(|(key, value)| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then_some((key, trimmed.to_string()))
        })
        .collect::<HashMap<_, _>>();

    let target = if domain.is_empty() {
        pre.domain_hints.first().cloned().unwrap_or_default()
    } else {
        domain.clone()
    };

    let hints = build_llm_hints(utterance, context, pre, &domain, &action, &params);
    Ok(OutcomeIntent {
        summary: if summary.is_empty() {
            format!("Intent from: {}", &response[..response.len().min(60)])
        } else {
            summary
        },
        plane: pre.plane,
        polarity: pre.polarity,
        domain_concept: domain.clone(),
        action: action.clone(),
        subject: None,
        steps: vec![OutcomeStep {
            action,
            target,
            params,
            notes: None,
        }],
        confidence,
        pending_clarifications: vec![],
        explain: build_llm_explain(context, utterance, confidence, pre.polarity, &domain),
        coder_handoff: build_llm_handoff(utterance, pre.polarity, &domain, &hints),
        hints,
    })
}

fn build_llm_hints(
    utterance: &str,
    context: &SageContext,
    pre: &SagePreClassification,
    domain: &str,
    action: &OutcomeAction,
    params: &HashMap<String, String>,
) -> UtteranceHints {
    UtteranceHints {
        raw_preview: utterance.trim()[..utterance.trim().len().min(80)].to_string(),
        subject_phrase: params
            .get("name")
            .cloned()
            .or_else(|| context.dominant_entity_name.clone()),
        explicit_domain_terms: (!domain.is_empty())
            .then_some(vec![domain.to_string()])
            .unwrap_or_else(|| pre.domain_hints.clone()),
        explicit_action_terms: vec![action.as_str().to_string()],
        scope_carry_forward_used: !context.last_intents.is_empty(),
        inventory_read: matches!(action, OutcomeAction::Read)
            && utterance.to_ascii_lowercase().contains("have"),
        structure_read: pre.sage_only,
        create_name_candidate: params.get("name").cloned(),
    }
}

fn build_llm_explain(
    context: &SageContext,
    utterance: &str,
    confidence: SageConfidence,
    polarity: IntentPolarity,
    domain: &str,
) -> SageExplain {
    SageExplain {
        understanding: if domain.is_empty() {
            format!("So you want to {}.", utterance.trim())
        } else {
            format!("So you want to work on {} for {}.", domain, utterance.trim())
        },
        mode: match polarity {
            IntentPolarity::Read | IntentPolarity::Ambiguous => "read_only".to_string(),
            IntentPolarity::Write => "confirmation_required".to_string(),
        },
        scope_summary: context.dominant_entity_name.clone(),
        confidence: confidence.as_str().to_string(),
        clarifications: vec![],
    }
}

fn build_llm_handoff(
    utterance: &str,
    polarity: IntentPolarity,
    domain: &str,
    hints: &UtteranceHints,
) -> CoderHandoff {
    let mut hint_terms = hints.explicit_domain_terms.clone();
    hint_terms.extend(hints.explicit_action_terms.iter().cloned());
    CoderHandoff {
        goal: if matches!(polarity, IntentPolarity::Write) {
            "prepare deterministic mutation proposal".to_string()
        } else {
            "serve current state safely".to_string()
        },
        intent_summary: utterance.trim()[..utterance.trim().len().min(80)].to_string(),
        required_outcome: if domain.is_empty() {
            "realize intended business outcome".to_string()
        } else {
            format!("realize intended {} outcome", domain)
        },
        constraints: vec![
            "respect_sem_os_surface".to_string(),
            "no_mutation_without_confirmation".to_string(),
        ],
        hint_terms,
        serve_safe: !matches!(polarity, IntentPolarity::Write),
        requires_confirmation: matches!(polarity, IntentPolarity::Write),
    }
}

fn apply_asymmetric_risk(raw: SageConfidence, polarity: IntentPolarity) -> SageConfidence {
    match polarity {
        IntentPolarity::Read => match raw {
            SageConfidence::Low => SageConfidence::Medium,
            other => other,
        },
        IntentPolarity::Write => match raw {
            SageConfidence::High => SageConfidence::High,
            _ => SageConfidence::Medium,
        },
        IntentPolarity::Ambiguous => raw,
    }
}

#[async_trait::async_trait]
impl SageEngine for LlmSage {
    async fn classify(&self, utterance: &str, context: &SageContext) -> Result<OutcomeIntent> {
        let pre = pre_classify(utterance, context);
        if pre.sage_only && !pre.domain_hints.is_empty() {
            return self.fallback.classify(utterance, context).await;
        }

        let prompt = self.build_user_prompt(utterance, context, &pre);
        let tool_result = match self
            .client
            .chat_with_tool(Self::system_prompt(), &prompt, &Self::tool_definition())
            .await
        {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    provider = self.client.provider_name(),
                    "LlmSage falling back to DeterministicSage"
                );
                return self.fallback.classify(utterance, context).await;
            }
        };

        let response = serde_json::to_string(&tool_result.arguments)
            .context("failed to serialize Sage tool response")?;
        match parse_sage_response(utterance, &response, context, &pre) {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    provider = self.client.provider_name(),
                    "LlmSage parse failed, falling back to DeterministicSage"
                );
                self.fallback.classify(utterance, context).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct StubLlmClient {
        result: serde_json::Value,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl LlmClient for StubLlmClient {
        async fn chat(&self, _system_prompt: &str, _user_prompt: &str) -> Result<String> {
            unreachable!()
        }

        async fn chat_json(&self, _system_prompt: &str, _user_prompt: &str) -> Result<String> {
            unreachable!()
        }

        async fn chat_with_tool(
            &self,
            _system_prompt: &str,
            _user_prompt: &str,
            tool: &ToolDefinition,
        ) -> Result<ob_agentic::llm_client::ToolCallResult> {
            if self.fail {
                anyhow::bail!("stub failure");
            }
            Ok(ob_agentic::llm_client::ToolCallResult {
                tool_name: tool.name.clone(),
                arguments: self.result.clone(),
            })
        }

        fn model_name(&self) -> &str {
            "stub"
        }

        fn provider_name(&self) -> &str {
            "stub"
        }
    }

    fn empty_ctx() -> SageContext {
        SageContext::default()
    }

    #[test]
    fn parse_sage_response_extracts_fields() {
        let response = r#"{"summary":"Create a CBU for Allianz in Luxembourg","domain":"cbu","action":"create","params":{"name":"Allianz Global Investors","jurisdiction":"LU"},"confidence":"high"}"#;
        let pre = SagePreClassification {
            plane: super::super::ObservationPlane::Instance,
            polarity: IntentPolarity::Write,
            polarity_clue: Some("create".to_string()),
            domain_hints: vec!["cbu".to_string()],
            sage_only: false,
        };

        let result = parse_sage_response(
            "create a cbu for Allianz in Luxembourg",
            response,
            &empty_ctx(),
            &pre,
        )
        .unwrap();
        assert_eq!(result.domain_concept, "cbu");
        assert_eq!(result.steps[0].params.len(), 2);
        assert!(result.steps[0].params.contains_key("name"));
        assert!(result.steps[0].params.contains_key("jurisdiction"));
    }

    #[test]
    fn asymmetric_risk_bumps_read_confidence() {
        assert_eq!(
            apply_asymmetric_risk(SageConfidence::Low, IntentPolarity::Read),
            SageConfidence::Medium
        );
    }

    #[test]
    fn asymmetric_risk_caps_write_confidence() {
        assert_eq!(
            apply_asymmetric_risk(SageConfidence::Low, IntentPolarity::Write),
            SageConfidence::Medium
        );
    }

    #[tokio::test]
    async fn llm_sage_uses_llm_payload() {
        let client = Arc::new(StubLlmClient {
            result: serde_json::json!({
                "summary": "Create a KYC case for this client",
                "domain": "kyc-case",
                "action": "create",
                "params": {"entity-name": "Allianz"},
                "confidence": "high"
            }),
            fail: false,
        });
        let sage = LlmSage::new(client);

        let outcome = sage
            .classify("open a kyc case for Allianz", &empty_ctx())
            .await
            .unwrap();

        assert_eq!(outcome.polarity, IntentPolarity::Write);
        assert_eq!(outcome.domain_concept, "kyc-case");
        assert_eq!(outcome.confidence, SageConfidence::High);
        assert_eq!(
            outcome.steps[0].params.get("entity-name").map(String::as_str),
            Some("Allianz")
        );
    }

    #[tokio::test]
    async fn llm_sage_falls_back_on_parse_error() {
        let client = Arc::new(StubLlmClient {
            result: serde_json::json!({"summary": 42}),
            fail: false,
        });
        let sage = LlmSage::new(client);

        let outcome = sage
            .classify("show me all funds", &empty_ctx())
            .await
            .unwrap();
        assert_eq!(outcome.polarity, IntentPolarity::Read);
    }

    #[tokio::test]
    async fn llm_sage_falls_back_on_client_error() {
        let client = Arc::new(StubLlmClient {
            result: serde_json::json!({}),
            fail: true,
        });
        let sage = LlmSage::new(client);

        let outcome = sage
            .classify("create a new fund", &empty_ctx())
            .await
            .unwrap();
        assert_eq!(outcome.polarity, IntentPolarity::Write);
    }
}
