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
use ob_agentic::llm_client::LlmClient;
use serde::{Deserialize, Serialize};

use crate::approval::ApprovalEvaluator;
use crate::blockers::BlockerDetector;
use crate::constellation::{ConstellationHydrator, ConstellationSnapshot, HydrationScope};
use crate::frontier::FrontierEngine;
use crate::goal_frame::GoalFrame;
use crate::index::SessionIndex;
use crate::knowledge::{active_verbs_query_for_index, KnowledgeResponse, SemOsKnowledgeClient};
use crate::motivation::MotivationPromptBuilder;

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
    knowledge: Option<Arc<dyn SemOsKnowledgeClient>>,
    hydrator: Option<Arc<dyn ConstellationHydrator>>,
}

impl PlanningLoop {
    /// Construct a planning loop. `llm_client`, `knowledge`, and
    /// `hydrator` are optional so the spike runs hermetically (no
    /// API key, no MCP transport).
    pub fn new(
        index: SessionIndex,
        llm_client: Option<Arc<dyn LlmClient>>,
        knowledge: Option<Arc<dyn SemOsKnowledgeClient>>,
        hydrator: Option<Arc<dyn ConstellationHydrator>>,
    ) -> Self {
        Self {
            index,
            llm_client,
            knowledge,
            hydrator,
        }
    }

    /// Read-only view of the index for handlers / audit.
    pub fn index(&self) -> &SessionIndex {
        &self.index
    }

    /// Optional knowledge client label for audit / diagnostics.
    pub fn knowledge_label(&self) -> Option<&str> {
        self.knowledge.as_ref().map(|k| k.provider_label())
    }

    /// Optional constellation hydrator label for audit / diagnostics.
    pub fn hydrator_label(&self) -> Option<&str> {
        self.hydrator.as_ref().map(|h| h.provider_label())
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
    ///
    /// `existing` is the frame bound to this session (Phase 3.1c).
    /// When `Some` and still mutable, the existing frame is reused
    /// (id + created_at + pack anchor preserved) and refined with
    /// the new utterance. Otherwise a fresh frame is seeded.
    pub async fn propose_draft(
        &self,
        utterance: &str,
        existing: Option<GoalFrame>,
    ) -> Result<PlanningOutcome> {
        let mut goal_frame = match existing {
            Some(mut frame) if frame.status.is_mutable() => {
                frame
                    .refine_with_utterance(utterance)
                    .expect("mutable status guarded above");
                frame
            }
            _ => GoalFrame::seed_for_spike(utterance, &self.index),
        };

        // Phase 3.2 — hydrate the constellation snapshot before the
        // LLM call. Stub hydrator returns empty; Phase 4 swaps for
        // the real MCP transport. Failures are non-fatal — fall
        // back to the pack allowlist.
        if let Some(hydrator) = self.hydrator.as_ref() {
            let scope = HydrationScope {
                workspace: &goal_frame.workspace,
                pack_id: &goal_frame.pack_id,
                constellation_id: None,
            };
            match hydrator.hydrate(scope).await {
                Ok(snapshot) => goal_frame.attach_constellation(snapshot),
                Err(error) => tracing::warn!(
                    target: "sage-acp",
                    %error,
                    "constellation hydrator failed — continuing without snapshot"
                ),
            }
        }

        // Phase 3.3 — compute the frontier from the manifest +
        // (possibly empty) constellation snapshot. Always runs:
        // pure compute, no IO, no async. Attaches to the goal frame
        // so downstream consumers (motivation prompt, audit) can
        // inspect what the planner thinks is open.
        let snapshot_ref = goal_frame
            .constellation
            .as_ref()
            .cloned()
            .unwrap_or_else(ConstellationSnapshot::empty);
        let frontier = FrontierEngine::compute(&self.index, &snapshot_ref);
        goal_frame.attach_frontier(frontier);

        // Phase 2.8 — exercise the knowledge surface so the seam is
        // demonstrably wired end-to-end. The spike client returns
        // Empty for every query; Phase 3.4 / 4 swap for the real
        // sem_os_mcp transport and hydrate constellation context
        // before the LLM call.
        if let (Some(client), Some(query)) = (
            self.knowledge.as_ref(),
            active_verbs_query_for_index(&self.index),
        ) {
            match client.query(query).await {
                Ok(KnowledgeResponse::Empty) => {
                    tracing::debug!(
                        target: "sage-acp",
                        "knowledge client returned Empty — using pack allowlist only"
                    );
                }
                Ok(response) => {
                    tracing::debug!(
                        target: "sage-acp",
                        ?response,
                        "knowledge client hydrated context"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        target: "sage-acp",
                        %error,
                        "knowledge client failed — continuing with pack allowlist"
                    );
                }
            }
        }

        // Phase 3.5 — build the pre-LLM blocker view so the
        // motivation prompt can surface blockers the LLM should
        // consider before picking a verb. The post-LLM detect call
        // below re-runs with the verb FQN to catch
        // UnsanctionedDraft, which can't be known before the LLM
        // returns.
        let pre_blockers = BlockerDetector::detect(
            &self.index,
            goal_frame.frontier.as_ref().expect("attached above"),
            &snapshot_ref,
            None,
        );

        let (verb_fqn, intent_summary, source) = match self.llm_client.as_ref() {
            Some(client) => {
                let prompt = MotivationPromptBuilder::build(
                    &self.index,
                    utterance,
                    goal_frame.frontier.as_ref().expect("attached above"),
                    Some(&pre_blockers),
                );
                let result = self.invoke_motivated_llm(client.as_ref(), &prompt).await?;
                if !self.index.is_verb_sanctioned(&result.verb_fqn) {
                    return Err(anyhow!(
                        "constrained-composition violation: LLM proposed '{}' which is not in \
                         pack '{}' allowlist (or is on the denylist)",
                        result.verb_fqn,
                        self.index.pack.id
                    ));
                }
                (result.verb_fqn, Some(result.intent_summary), DraftSource::LlmTool)
            }
            None => {
                let fallback = self
                    .deterministic_fallback(&goal_frame.refused_drafts)
                    .ok_or_else(|| {
                        anyhow!(
                            "no LLM client wired and pack '{}' has no sanctioned verbs (after \
                             excluding {} refused) to fall back on",
                            self.index.pack.id,
                            goal_frame.refused_drafts.len()
                        )
                    })?;
                (fallback, None, DraftSource::DeterministicFallback)
            }
        };

        if let Some(summary) = intent_summary {
            goal_frame.intent_summary = Some(summary);
            goal_frame.updated_at = chrono::Utc::now();
        }

        // Phase 3.4 — re-detect blockers with the verb FQN so the
        // UnsanctionedDraft kind can fire. Runs unconditionally;
        // the report is attached to the goal frame even when empty
        // so audit shape is stable.
        let blocker_report = BlockerDetector::detect(
            &self.index,
            goal_frame.frontier.as_ref().expect("attached above"),
            &snapshot_ref,
            Some(&verb_fqn),
        );
        goal_frame.attach_blockers(blocker_report);

        // Phase 3.6 — evaluate the approval decision from the pack
        // `risk_policy`. Attached even when not required so audit
        // shape is stable.
        let approval = ApprovalEvaluator::evaluate(
            &self.index,
            &goal_frame,
            goal_frame.frontier.as_ref().expect("attached above"),
            goal_frame.blockers.as_ref().expect("attached above"),
        );
        goal_frame.attach_approval(approval);

        Ok(PlanningOutcome {
            goal_frame,
            verb_fqn,
            source,
        })
    }

    fn deterministic_fallback(&self, refused: &[String]) -> Option<String> {
        self.index
            .allowed_verbs()
            .iter()
            .find(|v| !self.index.forbidden_verbs().contains(v) && !refused.contains(v))
            .cloned()
    }

    async fn invoke_motivated_llm(
        &self,
        client: &dyn LlmClient,
        prompt: &crate::motivation::MotivationPrompt,
    ) -> Result<LlmDraftResult> {
        let tool = MotivationPromptBuilder::tool_definition(&self.index);
        let result = client
            .chat_with_tool(&prompt.system, &prompt.user, &tool)
            .await?;
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
        let intent_summary = result
            .arguments
            .get("intent_summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(LlmDraftResult {
            verb_fqn,
            intent_summary,
        })
    }
}

#[derive(Debug, Clone)]
struct LlmDraftResult {
    verb_fqn: String,
    intent_summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::SessionIndex;
    use chrono::Utc;
    use ob_agentic::llm_client::{ToolCallResult, ToolDefinition};
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
        let loop_ = PlanningLoop::new(make_index(), None, None, None);
        let outcome = loop_.propose_draft("set up a book", None).await.unwrap();
        assert_eq!(outcome.verb_fqn, "cbu.create");
        assert_eq!(outcome.source, DraftSource::DeterministicFallback);
        assert_eq!(outcome.goal_frame.pack_id, "book-setup");
        assert_eq!(outcome.goal_frame.workspace, "cbu");
        assert!(outcome.goal_frame.intent_summary.is_none());
        assert!(outcome.goal_frame.id.starts_with("gf-"));
    }

    #[test]
    fn seed_goal_frame_captures_session_anchor() {
        let index = make_index();
        let frame = GoalFrame::seed_for_spike("attach a product to the new book", &index);
        assert_eq!(frame.utterance, "attach a product to the new book");
        assert_eq!(frame.pack_id, "book-setup");
        assert_eq!(frame.pack_hash, index.pack_hash);
        assert_eq!(frame.workspace, "cbu");
        assert!(frame.intent_summary.is_none(), "Phase 3.4 fills this");
        assert!(frame.id.starts_with("gf-"));
    }

    #[tokio::test]
    async fn llm_proposal_within_allowlist_is_accepted() {
        let llm: Arc<dyn LlmClient> = Arc::new(StubLlm {
            verb_fqn: "cbu.attach-product".to_string(),
        });
        let loop_ = PlanningLoop::new(make_index(), Some(llm), None, None);
        let outcome = loop_.propose_draft("attach the new product", None).await.unwrap();
        assert_eq!(outcome.verb_fqn, "cbu.attach-product");
        assert_eq!(outcome.source, DraftSource::LlmTool);
    }

    #[tokio::test]
    async fn llm_proposal_outside_allowlist_is_rejected() {
        let llm: Arc<dyn LlmClient> = Arc::new(StubLlm {
            verb_fqn: "cbu.delete".to_string(),
        });
        let loop_ = PlanningLoop::new(make_index(), Some(llm), None, None);
        let err = loop_
            .propose_draft("wipe the book", None)
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
        let loop_ = PlanningLoop::new(make_index(), Some(llm), None, None);
        let err = loop_
            .propose_draft("do something new", None)
            .await
            .expect_err("unlisted verb must reject");
        assert!(
            err.to_string().contains("constrained-composition violation"),
            "{err}"
        );
    }
}
