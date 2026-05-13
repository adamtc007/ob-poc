//! Sage planning loop — Phase 2.6 spike.
//!
//! The planning loop sits between the ACP `session/prompt` handler
//! and the LSP-shaped REPL channel. Its responsibility is to take a
//! raw utterance + a [`SessionIndex`] and return a draft proposal
//! the editor can review before any DSL hits the validator.
//!
//! ## Constrained composition guarantee
//!
//! Whether the draft comes from the LLM or from the deterministic
//! fallback, the verb FQN it emits is checked against
//! `SessionIndex::is_verb_sanctioned` before the outcome leaves the
//! loop. The planning loop will not return a draft that names a verb
//! the pack does not sanction (V&S §6.7 — no free-text DSL).
//!
//! ## Spike scope (Phase 2.6)
//!
//! - One round-trip per prompt. No iteration, no replanning, no
//!   blocker detection — those land in Phase 3.4–3.5.
//! - Hard-coded [`GoalFrame`] (placeholder for the real Motivated
//!   Sage shape in Phase 3.1).
//! - LLM call site is optional: if no `LlmClient` is wired, the loop
//!   falls back to a deterministic "first allowed verb" pick so the
//!   spike can run offline.
//! - When the LLM is wired, the prompt + tool schema are
//!   intentionally minimal so the Anthropic round-trip is small and
//!   replayable.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use ob_agentic::llm_client::{LlmClient, ToolDefinition};
use serde::{Deserialize, Serialize};

use crate::index::SessionIndex;

/// Placeholder for the Phase 3.1 [`GoalFrame`] shape. The spike fills
/// just the fields the audit emission slice (Phase 2.9) needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalFrame {
    /// Stable id the audit record correlates against.
    pub id: String,
    /// Raw utterance the user typed in the editor.
    pub utterance: String,
    /// Pack the session is anchored to.
    pub pack_id: String,
    /// Pack manifest hash (SHA-256 of raw YAML) — captured for
    /// replay-grade audit.
    pub pack_hash: String,
    /// When the frame was constructed.
    pub created_at: DateTime<Utc>,
}

/// Output of one planning round-trip.
///
/// Contains the proposed verb FQN, the goal frame it was drafted
/// against, and a tag identifying which call site produced it (LLM
/// vs deterministic fallback). The Phase 2.9 audit emitter logs the
/// outcome verbatim; the Phase 2.7 LSP-shaped channel client takes
/// `verb_fqn` and submits it for validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningOutcome {
    pub goal_frame: GoalFrame,
    pub verb_fqn: String,
    pub source: DraftSource,
}

/// Identifies which call site produced a [`PlanningOutcome::verb_fqn`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftSource {
    /// LLM proposed the verb FQN via the constrained tool schema.
    LlmTool,
    /// LLM was unavailable / no API key — the loop picked the first
    /// sanctioned verb from the pack allowlist.
    DeterministicFallback,
}

/// The Sage planning loop.
///
/// Holds the read-only index snapshot + an optional LLM client.
/// Constructed by the binary integrator at session start; reused
/// across `session/prompt` calls within the session lifetime.
pub struct PlanningLoop {
    index: SessionIndex,
    llm_client: Option<Arc<dyn LlmClient>>,
}

impl PlanningLoop {
    /// Construct a planning loop. `llm_client` is optional so the
    /// spike runs offline (no API key required).
    pub fn new(index: SessionIndex, llm_client: Option<Arc<dyn LlmClient>>) -> Self {
        Self { index, llm_client }
    }

    /// Read-only view of the index for handlers / audit.
    pub fn index(&self) -> &SessionIndex {
        &self.index
    }

    /// One round-trip — utterance → verb FQN.
    ///
    /// If [`Self::llm_client`] is `Some`, the loop calls
    /// `chat_with_tool` against a minimal tool schema constraining
    /// the LLM to pick a verb FQN from the pack's allowlist. If the
    /// LLM names a verb the pack does not sanction, the loop fails
    /// hard (does not silently fall back) — this preserves the
    /// constrained-composition invariant.
    ///
    /// If no LLM is wired, the loop picks the first sanctioned verb
    /// as a deterministic fallback so the spike can run end-to-end
    /// in CI / offline.
    pub async fn propose_draft(&self, utterance: &str) -> Result<PlanningOutcome> {
        let goal_frame = GoalFrame {
            id: format!("gf-{}", uuid::Uuid::new_v4()),
            utterance: utterance.to_string(),
            pack_id: self.index.pack.id.clone(),
            pack_hash: self.index.pack_hash.clone(),
            created_at: Utc::now(),
        };

        let (verb_fqn, source) = match self.llm_client.as_ref() {
            Some(client) => {
                let proposed = self.invoke_llm(client.as_ref(), utterance).await?;
                if !self.index.is_verb_sanctioned(&proposed) {
                    return Err(anyhow!(
                        "constrained-composition violation: LLM proposed '{proposed}' which is \
                         not in pack '{}' allowlist (or is on the denylist)",
                        self.index.pack.id
                    ));
                }
                (proposed, DraftSource::LlmTool)
            }
            None => {
                let fallback = self.deterministic_fallback().ok_or_else(|| {
                    anyhow!(
                        "no LLM client wired and pack '{}' has no sanctioned verbs to fall \
                         back on",
                        self.index.pack.id
                    )
                })?;
                (fallback, DraftSource::DeterministicFallback)
            }
        };

        Ok(PlanningOutcome {
            goal_frame,
            verb_fqn,
            source,
        })
    }

    fn deterministic_fallback(&self) -> Option<String> {
        self.index
            .allowed_verbs()
            .iter()
            .find(|v| !self.index.forbidden_verbs().contains(v))
            .cloned()
    }

    async fn invoke_llm(&self, client: &dyn LlmClient, utterance: &str) -> Result<String> {
        let tool = self.draft_tool_definition();
        let system_prompt = self.system_prompt();
        let user_prompt = format!(
            "Editor utterance: {utterance}\n\
             Pack: {pack_id}\n\
             Select the single most-applicable verb FQN from the allowlist.",
            pack_id = self.index.pack.id,
        );
        let result = client.chat_with_tool(&system_prompt, &user_prompt, &tool).await?;
        let verb_fqn = result
            .arguments
            .get("verb_fqn")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow!(
                    "LLM tool call returned no verb_fqn field; got: {}",
                    result.arguments
                )
            })?
            .to_string();
        Ok(verb_fqn)
    }

    fn system_prompt(&self) -> String {
        format!(
            "You are Sage, the constrained-composition drafter for a governed runbook \
             system. You may only select verbs from the pack's allowlist. Free-text DSL is \
             forbidden. Return exactly one verb FQN via the propose_verb tool.\n\
             \n\
             Allowed verbs: {allowed}\n\
             Forbidden verbs: {forbidden}",
            allowed = self.index.allowed_verbs().join(", "),
            forbidden = self.index.forbidden_verbs().join(", "),
        )
    }

    fn draft_tool_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "propose_verb".to_string(),
            description:
                "Select the verb FQN that best matches the editor utterance. The FQN MUST \
                 appear in the allowed-verbs list and MUST NOT appear in the forbidden-verbs \
                 list. The drafter will reject any FQN outside the sanctioned set."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "verb_fqn": {
                        "type": "string",
                        "description": "Fully-qualified verb name from the allowlist."
                    }
                },
                "required": ["verb_fqn"]
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::SessionIndex;
    use ob_agentic::llm_client::ToolCallResult;
    use ob_poc_journey::pack::load_pack_from_bytes;
    use ob_poc_types::session::kinds::WorkspaceKind;

    fn book_setup_manifest_yaml() -> &'static [u8] {
        br#"
id: book-setup
name: Book Setup
version: "0.1"
description: Spike fixture for planning loop tests.
invocation_phrases: []
required_context: []
optional_context: []
workspaces:
  - cbu
allowed_verbs:
  - cbu.create
  - cbu.attach-product
forbidden_verbs:
  - cbu.delete
required_questions: []
optional_questions: []
stop_rules: []
templates: []
section_layout: []
definition_of_done: []
progress_signals: []
"#
    }

    fn make_index() -> SessionIndex {
        let (pack, pack_hash) = load_pack_from_bytes(book_setup_manifest_yaml()).unwrap();
        SessionIndex {
            pack,
            pack_hash,
            workspace: WorkspaceKind::Cbu,
            loaded_at: Utc::now(),
        }
    }

    struct StubLlm {
        verb_fqn: String,
    }

    #[async_trait::async_trait]
    impl LlmClient for StubLlm {
        async fn chat(&self, _s: &str, _u: &str) -> Result<String> {
            Ok(String::new())
        }
        async fn chat_json(&self, _s: &str, _u: &str) -> Result<String> {
            Ok(String::new())
        }
        async fn chat_with_tool(
            &self,
            _s: &str,
            _u: &str,
            _t: &ToolDefinition,
        ) -> Result<ToolCallResult> {
            Ok(ToolCallResult {
                tool_name: "propose_verb".to_string(),
                arguments: serde_json::json!({"verb_fqn": self.verb_fqn}),
            })
        }
        fn model_name(&self) -> &str {
            "stub"
        }
        fn provider_name(&self) -> &str {
            "stub"
        }
    }

    #[tokio::test]
    async fn deterministic_fallback_picks_first_allowed_verb() {
        let loop_ = PlanningLoop::new(make_index(), None);
        let outcome = loop_.propose_draft("set up a book").await.unwrap();
        assert_eq!(outcome.verb_fqn, "cbu.create");
        assert_eq!(outcome.source, DraftSource::DeterministicFallback);
        assert_eq!(outcome.goal_frame.pack_id, "book-setup");
    }

    #[tokio::test]
    async fn llm_proposal_within_allowlist_is_accepted() {
        let llm: Arc<dyn LlmClient> = Arc::new(StubLlm {
            verb_fqn: "cbu.attach-product".to_string(),
        });
        let loop_ = PlanningLoop::new(make_index(), Some(llm));
        let outcome = loop_.propose_draft("attach the new product").await.unwrap();
        assert_eq!(outcome.verb_fqn, "cbu.attach-product");
        assert_eq!(outcome.source, DraftSource::LlmTool);
    }

    #[tokio::test]
    async fn llm_proposal_outside_allowlist_is_rejected() {
        let llm: Arc<dyn LlmClient> = Arc::new(StubLlm {
            verb_fqn: "cbu.delete".to_string(),
        });
        let loop_ = PlanningLoop::new(make_index(), Some(llm));
        let err = loop_
            .propose_draft("wipe the book")
            .await
            .expect_err("denylist hit must reject");
        assert!(
            err.to_string().contains("constrained-composition violation"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn llm_proposal_for_unsanctioned_verb_is_rejected() {
        let llm: Arc<dyn LlmClient> = Arc::new(StubLlm {
            verb_fqn: "cbu.invent-this".to_string(),
        });
        let loop_ = PlanningLoop::new(make_index(), Some(llm));
        let err = loop_
            .propose_draft("do something new")
            .await
            .expect_err("unlisted verb must reject");
        assert!(
            err.to_string().contains("constrained-composition violation"),
            "{err}"
        );
    }
}
