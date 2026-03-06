//! LlmSage — LLM-backed Sage implementation with deterministic fallback.
//!
//! The LLM never decides plane or polarity as the source of truth.
//! Those come from `pre_classify()`. The model fills in domain/action/steps
//! and may echo its own plane/polarity view for confidence scoring only.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use ob_agentic::llm_client::{LlmClient, ToolDefinition};
use serde::{Deserialize, Serialize};

use super::context::SageContext;
use super::deterministic::DeterministicSage;
use super::outcome::{
    Clarification, EntityRef, OutcomeAction, OutcomeIntent, OutcomeStep, SageConfidence,
};
use super::pre_classify::pre_classify;
use super::{IntentPolarity, ObservationPlane, SageEngine};

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
        r#"You are the Sage layer in a two-stage intent pipeline.
Return only structured function arguments.
You MUST NOT output DSL or verb FQNs.

ObservationPlane definitions:
- instance: operate on concrete records, cases, entities, funds, deals, sessions
- structure: operate on schemas, structures, taxonomies, templates, fund structures
- registry: operate on governance, registry, changesets, stewardship, audit metadata

IntentPolarity definitions:
- read: inspect, list, show, explain, search, report
- write: create, update, delete, assign, execute, request, upload, verify, run workflow actions
- ambiguous: unclear between read and write

Use the provided plane and polarity as hard constraints for the final answer.
Prefer concise summaries. If domain is unclear, return an empty string.
Only include steps when the utterance clearly implies multiple actions."#
    }

    fn build_user_prompt(
        &self,
        utterance: &str,
        context: &SageContext,
        pre: &super::pre_classify::SagePreClassification,
    ) -> String {
        format!(
            "utterance: {utterance}\nplane: {}\npolarity: {}\ndomain_hints: {:?}\nstage_focus: {:?}\ngoals: {:?}\nentity_kind: {:?}\ndominant_entity_name: {:?}\nlast_intents: {:?}",
            pre.plane.as_str(),
            pre.polarity.as_str(),
            pre.domain_hints,
            context.stage_focus,
            context.goals,
            context.entity_kind,
            context.dominant_entity_name,
            context.last_intents,
        )
    }

    fn tool_definition() -> ToolDefinition {
        ToolDefinition {
            name: "classify_sage_outcome".to_string(),
            description: "Classify natural-language intent into the Sage outcome schema without DSL or verb names".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string" },
                    "domain_concept": { "type": "string" },
                    "action": { "type": "string" },
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "action": { "type": "string" },
                                "target": { "type": "string" },
                                "params": {
                                    "type": "object",
                                    "additionalProperties": { "type": "string" }
                                },
                                "notes": { "type": ["string", "null"] }
                            },
                            "required": ["action", "target", "params"]
                        }
                    },
                    "pending_clarifications": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "question": { "type": "string" },
                                "options": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "clarifies": { "type": "string" }
                            },
                            "required": ["question", "options", "clarifies"]
                        }
                    },
                    "subject": {
                        "type": ["object", "null"],
                        "properties": {
                            "mention": { "type": "string" },
                            "kind_hint": { "type": ["string", "null"] }
                        },
                        "required": ["mention"]
                    },
                    "llm_plane": { "type": ["string", "null"] },
                    "llm_polarity": { "type": ["string", "null"] }
                },
                "required": ["summary", "domain_concept", "action", "steps", "pending_clarifications"]
            }),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct LlmOutcomePayload {
    summary: String,
    #[serde(default)]
    domain_concept: String,
    action: String,
    #[serde(default)]
    steps: Vec<LlmOutcomeStep>,
    #[serde(default)]
    pending_clarifications: Vec<Clarification>,
    #[serde(default)]
    subject: Option<LlmEntityRef>,
    #[serde(default)]
    llm_plane: Option<String>,
    #[serde(default)]
    llm_polarity: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LlmOutcomeStep {
    action: String,
    target: String,
    #[serde(default)]
    params: HashMap<String, String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LlmEntityRef {
    mention: String,
    #[serde(default)]
    kind_hint: Option<String>,
}

fn parse_action(raw: &str) -> OutcomeAction {
    match raw.trim().to_ascii_lowercase().as_str() {
        "read" | "list" | "show" | "describe" | "inspect" => OutcomeAction::Read,
        "create" | "open" => OutcomeAction::Create,
        "update" | "change" | "modify" => OutcomeAction::Update,
        "delete" | "remove" | "archive" => OutcomeAction::Delete,
        "assign" | "link" | "attach" => OutcomeAction::Assign,
        "import" | "sync" | "load" | "enrich" => OutcomeAction::Import,
        "search" | "find" | "discover" | "trace" => OutcomeAction::Search,
        "compute" | "calculate" | "analyze" | "analyse" => OutcomeAction::Compute,
        "publish" | "approve" | "promote" => OutcomeAction::Publish,
        other => OutcomeAction::Other(other.to_string()),
    }
}

fn parse_plane(raw: Option<&str>) -> Option<ObservationPlane> {
    match raw?.trim().to_ascii_lowercase().as_str() {
        "instance" => Some(ObservationPlane::Instance),
        "structure" => Some(ObservationPlane::Structure),
        "registry" => Some(ObservationPlane::Registry),
        _ => None,
    }
}

fn parse_polarity(raw: Option<&str>) -> Option<IntentPolarity> {
    match raw?.trim().to_ascii_lowercase().as_str() {
        "read" => Some(IntentPolarity::Read),
        "write" => Some(IntentPolarity::Write),
        "ambiguous" => Some(IntentPolarity::Ambiguous),
        _ => None,
    }
}

fn confidence_from_alignment(
    pre_plane: ObservationPlane,
    pre_polarity: IntentPolarity,
    llm_plane: Option<ObservationPlane>,
    llm_polarity: Option<IntentPolarity>,
    domain_concept: &str,
) -> SageConfidence {
    let plane_match = llm_plane.is_none_or(|plane| plane == pre_plane);
    let polarity_match = llm_polarity.is_none_or(|polarity| polarity == pre_polarity);

    if plane_match && polarity_match && !domain_concept.is_empty() {
        SageConfidence::High
    } else if plane_match || polarity_match || !domain_concept.is_empty() {
        SageConfidence::Medium
    } else {
        SageConfidence::Low
    }
}

#[async_trait::async_trait]
impl SageEngine for LlmSage {
    async fn classify(&self, utterance: &str, context: &SageContext) -> Result<OutcomeIntent> {
        let pre = pre_classify(utterance, context);
        let user_prompt = self.build_user_prompt(utterance, context, &pre);
        let tool = Self::tool_definition();

        let tool_result = match self
            .client
            .chat_with_tool(Self::system_prompt(), &user_prompt, &tool)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(error = %error, provider = self.client.provider_name(), "LlmSage falling back to DeterministicSage");
                return self.fallback.classify(utterance, context).await;
            }
        };

        let payload: LlmOutcomePayload = match serde_json::from_value(tool_result.arguments)
            .context("failed to parse LlmSage tool output")
        {
            Ok(payload) => payload,
            Err(error) => {
                tracing::warn!(error = %error, provider = self.client.provider_name(), "LlmSage parse failed, falling back to DeterministicSage");
                return self.fallback.classify(utterance, context).await;
            }
        };

        let llm_plane = parse_plane(payload.llm_plane.as_deref());
        let llm_polarity = parse_polarity(payload.llm_polarity.as_deref());
        let confidence = confidence_from_alignment(
            pre.plane,
            pre.polarity,
            llm_plane,
            llm_polarity,
            &payload.domain_concept,
        );

        Ok(OutcomeIntent {
            summary: if payload.summary.trim().is_empty() {
                format!("Intent from: {}", &utterance[..utterance.len().min(60)])
            } else {
                payload.summary
            },
            plane: pre.plane,
            polarity: pre.polarity,
            domain_concept: payload.domain_concept,
            action: parse_action(&payload.action),
            subject: payload.subject.map(|subject| EntityRef {
                mention: subject.mention,
                kind_hint: subject.kind_hint,
                uuid: None,
            }),
            steps: payload
                .steps
                .into_iter()
                .map(|step| OutcomeStep {
                    action: parse_action(&step.action),
                    target: step.target,
                    params: step.params,
                    notes: step.notes,
                })
                .collect(),
            confidence,
            pending_clarifications: payload.pending_clarifications,
        })
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

    #[tokio::test]
    async fn llm_sage_uses_llm_payload() {
        let client = Arc::new(StubLlmClient {
            result: serde_json::json!({
                "summary": "Open a KYC case and collect documents",
                "domain_concept": "kyc",
                "action": "create",
                "steps": [
                    {"action": "create", "target": "case", "params": {}, "notes": null},
                    {"action": "collect", "target": "document", "params": {"scope": "identity"}, "notes": "KYC pack"}
                ],
                "pending_clarifications": [],
                "subject": {"mention": "this client", "kind_hint": "entity"},
                "llm_plane": "instance",
                "llm_polarity": "write"
            }),
            fail: false,
        });
        let sage = LlmSage::new(client);

        let outcome = sage
            .classify("open a kyc case and collect documents", &empty_ctx())
            .await
            .unwrap();

        assert_eq!(outcome.plane, ObservationPlane::Instance);
        assert_eq!(outcome.polarity, IntentPolarity::Write);
        assert_eq!(outcome.domain_concept, "kyc");
        assert_eq!(outcome.confidence, SageConfidence::High);
        assert_eq!(outcome.steps.len(), 2);
        assert_eq!(
            outcome
                .subject
                .as_ref()
                .and_then(|s| s.kind_hint.as_deref()),
            Some("entity")
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
        assert_eq!(outcome.confidence, SageConfidence::Low);
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
        assert_eq!(outcome.confidence, SageConfidence::Low);
        assert_eq!(outcome.polarity, IntentPolarity::Write);
    }
}
