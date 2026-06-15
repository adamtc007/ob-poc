//! Motivation prompt template — Phase 3.5 (C-10).
//!
//! Replaces the Phase 2 spike's hard-coded "pick a verb" system
//! prompt with a richer template that grounds the LLM call in the
//! agent's current view of the world: goal frame + frontier +
//! blockers + (eventual) constellation excerpts. The output is a
//! [`MotivationPrompt`] the planning loop passes to `chat_with_tool`.
//!
//! ## What changes vs. Phase 2.6
//!
//! Phase 2.6's system prompt was a one-line allowlist instruction.
//! Phase 3.5:
//! - Names what the pack is for (description + invocation phrases).
//! - Surfaces open frontier items so the LLM knows what's still
//!   required.
//! - Surfaces blockers with remediation hints so the LLM picks a
//!   verb that *unblocks* rather than one that's syntactically
//!   sanctioned but stuck.
//! - Asks the LLM to also return a one-sentence `intent_summary`
//!   so the goal frame becomes self-describing in audit.
//!
//! ## Tool schema
//!
//! [`MotivationPromptBuilder::tool_definition`] returns the
//! `propose_verb` schema extended with `intent_summary` (string,
//! required). The planning loop reads both fields and threads
//! `intent_summary` onto the goal frame.

use ob_agentic::llm_client::ToolDefinition;

use crate::blockers::{BlockerKind, BlockerReport};
use crate::frontier::{Frontier, FrontierItemKind};
use crate::index::SessionIndex;

/// System + user prompts for one motivation round-trip.
#[derive(Debug, Clone)]
pub struct MotivationPrompt {
    pub system: String,
    pub user: String,
}

/// Builds the motivation prompt from the agent's current view.
pub struct MotivationPromptBuilder;

impl MotivationPromptBuilder {
    /// Build the prompt for a draft round-trip.
    pub fn build(
        index: &SessionIndex,
        utterance: &str,
        frontier: &Frontier,
        blockers: Option<&BlockerReport>,
    ) -> MotivationPrompt {
        MotivationPrompt {
            system: render_system_prompt(index),
            user: render_user_prompt(index, utterance, frontier, blockers),
        }
    }

    /// Tool definition the planning loop uses with
    /// `chat_with_tool`. Returns a structured `{verb_fqn,
    /// intent_summary}` object. Both fields required so the LLM
    /// cannot return one without the other.
    pub fn tool_definition(index: &SessionIndex) -> ToolDefinition {
        Self::tool_definition_with_allowlist(index.allowed_verbs())
    }

    /// Tool definition constrained to a precomputed effective
    /// allowlist (Phase 4.6). The planning loop intersects the pack
    /// allowlist with the substrate's active-verb-surface and the
    /// user's refused-draft set before calling this, so the JSON
    /// Schema enum the LLM sees is the *exact* sanctioned set for
    /// this round. Falls back to the pack allowlist via
    /// [`tool_definition`] when callers don't have an effective
    /// allowlist.
    pub fn tool_definition_with_allowlist(allowlist: &[String]) -> ToolDefinition {
        let enum_values: Vec<serde_json::Value> = allowlist
            .iter()
            .map(|fqn| serde_json::Value::String(fqn.clone()))
            .collect();
        ToolDefinition {
            name: "propose_verb".to_string(),
            description: "Select the verb FQN that best advances the frontier given the blockers, \
                 plus a one-sentence intent summary explaining the choice. The verb_fqn \
                 MUST be drawn from the enum below — the drafter rejects any FQN outside \
                 it."
            .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "verb_fqn": {
                        "type": "string",
                        "enum": enum_values,
                        "description":
                            "Fully-qualified verb. Must be one of the listed enum values."
                    },
                    "intent_summary": {
                        "type": "string",
                        "description":
                            "One sentence summarising what this verb is intended to \
                             accomplish in the current session context."
                    }
                },
                "required": ["verb_fqn", "intent_summary"]
            }),
        }
    }
}

fn render_system_prompt(index: &SessionIndex) -> String {
    let invocation = if index.pack.invocation_phrases.is_empty() {
        String::new()
    } else {
        format!(
            "\nThis pack is typically invoked when the user says things like:\n{}\n",
            index
                .pack
                .invocation_phrases
                .iter()
                .map(|p| format!("- \"{p}\""))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    format!(
        "You are Sage, the constrained-composition drafter for a governed runbook system. \
         The current session is anchored to pack '{pack_id}' (workspace {workspace}).\n\
         \n\
         Pack purpose: {description}\n{invocation}\n\
         Constrained-composition rule: you may only select verbs from the pack's allowlist. \
         Free-text DSL is forbidden. Return exactly one verb FQN (and a brief intent \
         summary) via the propose_verb tool. The drafter rejects any FQN outside the \
         sanctioned set.\n\
         \n\
         Allowed verbs: {allowed}\n\
         Forbidden verbs: {forbidden}",
        pack_id = index.pack.id,
        workspace = match index.workspace {
            ob_poc_types::session::kinds::WorkspaceKind::Cbu => "cbu",
            ob_poc_types::session::kinds::WorkspaceKind::Kyc => "kyc",
            ob_poc_types::session::kinds::WorkspaceKind::Deal => "deal",
            ob_poc_types::session::kinds::WorkspaceKind::ProductMaintenance => "product",
            ob_poc_types::session::kinds::WorkspaceKind::InstrumentMatrix => "instrument",
            ob_poc_types::session::kinds::WorkspaceKind::Catalogue => "catalogue",
            ob_poc_types::session::kinds::WorkspaceKind::LifecycleResources => "lifecycle",
            ob_poc_types::session::kinds::WorkspaceKind::OnBoarding => "onboarding",
            ob_poc_types::session::kinds::WorkspaceKind::SemOsMaintenance => "semos_maintenance",
            ob_poc_types::session::kinds::WorkspaceKind::Bpmn => "bpmn",
        },
        description = index.pack.description,
        allowed = index.allowed_verbs().join(", "),
        forbidden = if index.forbidden_verbs().is_empty() {
            "(none)".to_string()
        } else {
            index.forbidden_verbs().join(", ")
        },
    )
}

fn render_user_prompt(
    index: &SessionIndex,
    utterance: &str,
    frontier: &Frontier,
    blockers: Option<&BlockerReport>,
) -> String {
    let mut out = String::new();
    out.push_str("Editor utterance: ");
    out.push_str(utterance);
    out.push_str("\n\n");

    out.push_str(&format!(
        "Frontier ({} open of {} total items):\n",
        frontier.open_count(),
        frontier.items.len()
    ));
    for item in frontier.items.iter().filter(|i| !i.satisfied) {
        let kind_tag = match item.kind {
            FrontierItemKind::DefinitionOfDone => "DoD",
            FrontierItemKind::ProgressSignal => "signal",
            FrontierItemKind::RequiredQuestion => "question",
        };
        out.push_str(&format!("- [{kind_tag}] {}\n", item.description));
    }
    if frontier.open_count() == 0 {
        out.push_str("- (frontier fully satisfied)\n");
    }
    out.push('\n');

    if let Some(report) = blockers {
        if !report.is_empty() {
            out.push_str("Blockers:\n");
            for blocker in &report.blockers {
                let kind_tag = match blocker.kind {
                    BlockerKind::RequiredQuestionUnanswered => "missing_answer",
                    BlockerKind::UnsanctionedDraft => "unsanctioned",
                    BlockerKind::EmptyConstellation => "empty_state",
                    BlockerKind::CrossWorkspaceState => "cross_workspace",
                    BlockerKind::PendingRemediation => "pending_remediation",
                };
                out.push_str(&format!(
                    "- [{kind_tag}] {} (blocks: {})\n",
                    blocker.description, blocker.blocked_item
                ));
            }
            out.push('\n');
        }
    }

    out.push_str(&format!(
        "Select the single most-applicable verb FQN from the pack '{}' allowlist. The \
         verb you choose should make progress against the open frontier and (where \
         possible) unblock a current blocker.",
        index.pack.id
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constellation::ConstellationSnapshot;
    use crate::frontier::FrontierEngine;
    use chrono::Utc;
    use ob_poc_journey::pack::load_pack_from_bytes;
    use ob_poc_types::session::kinds::WorkspaceKind;

    fn manifest_yaml() -> &'static [u8] {
        br#"
id: book-setup
name: Book Setup
version: "0.1"
description: Spike fixture for motivation prompt
invocation_phrases:
  - "set up book"
  - "open a book"
required_context: []
optional_context: []
workspaces:
  - cbu
allowed_verbs:
  - cbu.create
  - cbu.attach-product
forbidden_verbs:
  - cbu.delete
required_questions:
  - field: jurisdiction
    prompt: Which jurisdiction?
optional_questions: []
stop_rules: []
templates: []
section_layout: []
definition_of_done:
  - "CBU created"
progress_signals: []
"#
    }

    fn make_index() -> SessionIndex {
        let (pack, pack_hash) = load_pack_from_bytes(manifest_yaml()).unwrap();
        SessionIndex {
            pack,
            pack_hash,
            workspace: WorkspaceKind::Cbu,
            loaded_at: Utc::now(),
        }
    }

    #[test]
    fn system_prompt_lists_allowed_and_forbidden_verbs() {
        let index = make_index();
        let prompt = MotivationPromptBuilder::build(
            &index,
            "set up a book",
            &FrontierEngine::compute(&index, &ConstellationSnapshot::empty()),
            None,
        );
        assert!(prompt.system.contains("cbu.create"));
        assert!(prompt.system.contains("cbu.attach-product"));
        assert!(prompt.system.contains("cbu.delete"));
        assert!(prompt.system.contains("constrained-composition"));
        assert!(prompt.system.contains("book-setup"));
        // Invocation phrases surface.
        assert!(prompt.system.contains("set up book"));
    }

    #[test]
    fn user_prompt_lists_open_frontier_items_and_blockers() {
        let index = make_index();
        let snapshot = ConstellationSnapshot::empty();
        let frontier = FrontierEngine::compute(&index, &snapshot);
        let blockers = crate::blockers::BlockerDetector::detect(
            &index,
            &frontier,
            &snapshot,
            Some("cbu.create"),
        );

        let prompt =
            MotivationPromptBuilder::build(&index, "set up a book", &frontier, Some(&blockers));
        assert!(prompt.user.contains("Editor utterance: set up a book"));
        assert!(prompt.user.contains("Frontier"));
        assert!(prompt.user.contains("Which jurisdiction?"));
        assert!(prompt.user.contains("Blockers"));
        assert!(prompt.user.contains("missing_answer"));
        assert!(prompt.user.contains("empty_state"));
    }

    #[test]
    fn tool_definition_requires_both_fields() {
        let tool = MotivationPromptBuilder::tool_definition(&make_index());
        assert_eq!(tool.name, "propose_verb");
        let required = tool.parameters["required"].as_array().unwrap();
        assert_eq!(required.len(), 2);
        let required_set: std::collections::HashSet<&str> =
            required.iter().filter_map(|v| v.as_str()).collect();
        assert!(required_set.contains("verb_fqn"));
        assert!(required_set.contains("intent_summary"));
    }
}
