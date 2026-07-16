//! Response adapter — converts `ReplResponseV2` to `ChatResponse`.
//!
//! The frontend consumes `ChatResponse` (from `ob-poc-types`). The REPL V2
//! orchestrator produces `ReplResponseV2`. This adapter bridges the two so the
//! frontend works unchanged as we migrate to the REPL V2 session model.

use ob_poc_types::chat::{ChatResponse, DraftProposalPayload, SessionStateEnum};
use ob_poc_types::decision::{
    ClarificationPayload, DecisionKind, DecisionPacket, DecisionTrace, GroupClarificationPayload,
    SessionStateView, UserChoice,
};
use ob_poc_types::disambiguation::{VerbDisambiguationRequest, VerbOption};
use uuid::Uuid;

use crate::repl::response_v2::{ReplResponseKindV2, ReplResponseV2};
use crate::repl::types_v2::ReplStateV2;

/// Convert a REPL V2 response to a ChatResponse for the frontend.
pub fn repl_to_chat_response(resp: ReplResponseV2, session_id: Uuid) -> ChatResponse {
    let session_state = repl_state_to_session_state(&resp.state);
    let session_feedback = resp
        .session_feedback
        .as_ref()
        .and_then(|fb| serde_json::to_value(fb).ok());

    let mut chat = ChatResponse {
        message: resp.message.clone(),
        dsl: None,
        session_state,
        commands: None,
        disambiguation_request: None,
        verb_disambiguation: None,
        intent_tier: None,
        unresolved_refs: None,
        current_ref_index: None,
        dsl_hash: None,
        decision: None,
        available_verbs: None,
        surface_fingerprint: None,
        sage_explain: None,
        drafter_proposal: None,
        discovery_bootstrap: None,
        parked_entries: None,
        onboarding_state: None,
        runbook_plan: None,
        session_feedback,
        narration: resp.narration.clone(),
        // R8 Phase B (2026-05-11): typed projection of the ACP DAG
        // semantic resolution into `acp_trace`. Replaces Phase A's
        // pre-built `ChatResponse` carrier. The typed summary mirrors
        // the ~30-key flat shape the chat UI's `AcpTraceCard` consumes.
        acp_trace: resp
            .acp_dag_semantic
            .as_ref()
            .map(ob_poc_boundary::acp_dag_semantic::acp_chat_trace_summary_typed),
        // Phase A.2 (F5 follow-on): forward the turn-level correlation id.
        trace_id: resp.trace_id,
        bpmn_form: resp.bpmn_form.clone(),
    };

    match resp.kind {
        ReplResponseKindV2::ScopeRequired { ref prompt } => {
            chat.decision = Some(build_decision(
                &format!("scope-{session_id}"),
                DecisionKind::ClarifyGroup,
                session_id,
                prompt,
                vec![], // Bootstrap handles free-text; no fixed choices
                "session_bootstrap",
            ));
        }

        ReplResponseKindV2::WorkspaceOptions { ref workspaces } => {
            let choices = workspaces
                .iter()
                .enumerate()
                .map(|(i, ws)| UserChoice {
                    id: format!("{}", i + 1),
                    label: ws.label.clone(),
                    description: ws.description.clone(),
                    is_escape: false,
                })
                .collect();
            chat.decision = Some(build_decision(
                &format!("ws-{session_id}"),
                DecisionKind::ClarifyWorkspace,
                session_id,
                &resp.message,
                choices,
                "workspace_gate",
            ));
        }

        ReplResponseKindV2::ConstellationMapOptions { ref options } => {
            let choices = options
                .iter()
                .enumerate()
                .map(|(i, option)| UserChoice {
                    id: format!("{}", i + 1),
                    label: option.label.clone(),
                    description: format!(
                        "{} | {} | {}",
                        option.constellation_map, option.jurisdiction, option.description
                    ),
                    is_escape: false,
                })
                .collect();
            chat.decision = Some(build_decision(
                &format!("cbu-map-{session_id}"),
                DecisionKind::ClarifyWorkspace,
                session_id,
                &resp.message,
                choices,
                "cbu_constellation_map_gate",
            ));
        }

        ReplResponseKindV2::JourneyOptions { ref packs } => {
            let choices = packs
                .iter()
                .enumerate()
                .map(|(i, p)| UserChoice {
                    id: format!("{}", i + 1),
                    label: p.pack_name.clone(),
                    description: p.description.clone(),
                    is_escape: false,
                })
                .collect();
            chat.decision = Some(build_decision(
                &format!("jp-{session_id}"),
                DecisionKind::ClarifyJourney,
                session_id,
                &resp.message,
                choices,
                "journey_gate",
            ));
        }

        ReplResponseKindV2::SentencePlayback {
            ref sentence,
            ref verb,
            ..
        } => {
            chat.drafter_proposal = Some(DraftProposalPayload {
                verb_fqn: Some(verb.clone()),
                dsl: Some(sentence.clone()),
                change_summary: vec![resp.message.clone()],
                requires_confirmation: true,
                ready_to_execute: true,
            });
        }

        ReplResponseKindV2::Clarification {
            ref question,
            ref options,
        } => {
            chat.verb_disambiguation = Some(VerbDisambiguationRequest {
                request_id: format!("clar-{session_id}"),
                original_input: String::new(),
                options: options
                    .iter()
                    .map(|o| VerbOption {
                        verb_fqn: o.verb_fqn.clone(),
                        description: o.description.clone(),
                        example: String::new(),
                        score: o.score,
                        matched_phrase: None,
                        domain_label: None,
                        category_label: None,
                        suggested_utterance: None,
                        verb_kind: None,
                        differentiation: None,
                        requires_state: None,
                        produces_state: None,
                        scope: None,
                        step_count: None,
                        target_entity_kind: None,
                        constellation_slot: None,
                        entity_context: None,
                        target_entity_name: None,
                    })
                    .collect(),
                prompt: question.clone(),
            });
        }

        ReplResponseKindV2::StepProposals { ref proposals, .. } => {
            if let Some(top) = proposals.first() {
                chat.drafter_proposal = Some(DraftProposalPayload {
                    verb_fqn: Some(top.verb.clone()),
                    dsl: Some(top.sentence.clone()),
                    change_summary: vec![top.sentence.clone()],
                    requires_confirmation: true,
                    ready_to_execute: true,
                });
            }
        }

        ReplResponseKindV2::Executed { ref results } => {
            // If a verb outcome carries a bpmn_form key (workflow.start-process
            // pattern), surface it as ChatMessage.bpmn_form for the React cockpit.
            if chat.bpmn_form.is_none() {
                for step in results {
                    if let Some(result) = &step.result {
                        if let Some(form_val) = result.get("bpmn_form").filter(|v| !v.is_null()) {
                            chat.bpmn_form = serde_json::from_value(form_val.clone()).ok();
                            break;
                        }
                    }
                }
            }
        }

        ReplResponseKindV2::RunbookSummary { .. }
        | ReplResponseKindV2::Parked { .. }
        | ReplResponseKindV2::Question { .. }
        | ReplResponseKindV2::Info { .. }
        | ReplResponseKindV2::Prompt { .. }
        | ReplResponseKindV2::Error { .. } => {
            // Message field already carries the human-readable content.
        }

        // R8 single-path unification (2026-05-11): ACP-resolved short-
        // circuit. The orchestrator's first-step ACP resolution produced
        // a Slice 1 pack-bound match. `resp.message` already carries the
        // human-readable text. `chat.acp_trace` is already populated from
        // `resp.acp_dag_semantic` at the top of this function. Project
        // the typed draft DSL into `chat.dsl` for the chat UI.
        ReplResponseKindV2::AcpResolved { ref dsl } => {
            chat.session_state = SessionStateEnum::Scoped;
            if let Some(source) = dsl {
                chat.dsl = Some(ob_poc_types::DslState {
                    source: Some(source.clone()),
                    ast: None,
                    can_execute: false,
                    bindings: Default::default(),
                });
            }
        }
    }

    chat
}

/// Map REPL V2 state to the frontend's SessionStateEnum.
fn repl_state_to_session_state(state: &ReplStateV2) -> SessionStateEnum {
    match state {
        ReplStateV2::ScopeGate { .. } | ReplStateV2::WorkspaceSelection { .. } => {
            SessionStateEnum::New
        }
        ReplStateV2::ConstellationMapSelection { .. }
        | ReplStateV2::JourneySelection { .. }
        | ReplStateV2::InPack { .. } => SessionStateEnum::Scoped,
        ReplStateV2::Clarifying { .. } | ReplStateV2::PackMismatchConfirm { .. } => {
            SessionStateEnum::PendingValidation
        }
        ReplStateV2::SentencePlayback { .. } | ReplStateV2::RunbookEditing => {
            SessionStateEnum::ReadyToExecute
        }
        ReplStateV2::Executing { .. } => SessionStateEnum::Executing,
    }
}

/// Build a DecisionPacket for the tollgate gates.
fn build_decision(
    packet_id: &str,
    kind: DecisionKind,
    session_id: Uuid,
    prompt: &str,
    choices: Vec<UserChoice>,
    reason: &str,
) -> DecisionPacket {
    DecisionPacket {
        packet_id: packet_id.to_string(),
        kind,
        session: SessionStateView {
            session_id: Some(session_id),
            ..Default::default()
        },
        utterance: String::new(),
        payload: ClarificationPayload::Group(GroupClarificationPayload { options: vec![] }),
        prompt: prompt.to_string(),
        choices,
        best_plan: None,
        alternatives: vec![],
        requires_confirm: false,
        confirm_token: None,
        trace: default_trace(reason),
    }
}

fn default_trace(reason: &str) -> DecisionTrace {
    DecisionTrace {
        config_version: String::new(),
        entity_snapshot_hash: None,
        lexicon_snapshot_hash: None,
        semantic_lane_enabled: false,
        embedding_model_id: None,
        verb_margin: 0.0,
        scope_margin: 0.0,
        kind_margin: 0.0,
        decision_reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repl::types_v2::{
        ConstellationMapOption, PackCandidate, WorkspaceKind, WorkspaceOption,
    };

    #[test]
    fn scope_required_maps_to_decision() {
        let resp = ReplResponseV2 {
            state: ReplStateV2::ScopeGate {
                pending_input: None,
                candidates: None,
            },
            kind: ReplResponseKindV2::ScopeRequired {
                prompt: "Which client group?".to_string(),
            },
            message: "Which client group would you like to work with?".to_string(),
            runbook_summary: None,
            step_count: 0,
            session_feedback: None,
            narration: None,
            trace_id: None,
            acp_dag_semantic: None,
            bpmn_form: None,
        };
        let chat = repl_to_chat_response(resp, Uuid::nil());
        assert!(chat.decision.is_some());
        let d = chat.decision.unwrap();
        assert!(matches!(d.kind, DecisionKind::ClarifyGroup));
        assert!(matches!(chat.session_state, SessionStateEnum::New));
    }

    #[test]
    fn workspace_options_maps_to_decision() {
        let resp = ReplResponseV2 {
            state: ReplStateV2::WorkspaceSelection { workspaces: vec![] },
            kind: ReplResponseKindV2::WorkspaceOptions {
                workspaces: vec![
                    WorkspaceOption {
                        workspace: WorkspaceKind::Cbu,
                        label: "CBU".to_string(),
                        description: "Client Business Unit management".to_string(),
                    },
                    WorkspaceOption {
                        workspace: WorkspaceKind::Kyc,
                        label: "KYC".to_string(),
                        description: "Know Your Customer".to_string(),
                    },
                ],
            },
            message: "Select a workspace.".to_string(),
            runbook_summary: None,
            step_count: 0,
            session_feedback: None,
            narration: None,
            trace_id: None,
            acp_dag_semantic: None,
            bpmn_form: None,
        };
        let chat = repl_to_chat_response(resp, Uuid::nil());
        assert!(chat.decision.is_some());
        let d = chat.decision.unwrap();
        assert!(matches!(d.kind, DecisionKind::ClarifyWorkspace));
        assert_eq!(d.choices.len(), 2);
        assert_eq!(d.choices[0].label, "CBU");
    }

    #[test]
    fn constellation_map_options_maps_to_decision() {
        let resp = ReplResponseV2 {
            state: ReplStateV2::ConstellationMapSelection { options: vec![] },
            kind: ReplResponseKindV2::ConstellationMapOptions {
                options: vec![ConstellationMapOption {
                    constellation_map: "struct.ie.ucits.icav".to_string(),
                    constellation_family: "ie_icav".to_string(),
                    label: "Ireland UCITS ICAV".to_string(),
                    description: "Ireland UCITS ICAV onboarding constellation".to_string(),
                    jurisdiction: "IE".to_string(),
                }],
            },
            message: "Choose the CBU structure DAG.".to_string(),
            runbook_summary: None,
            step_count: 0,
            session_feedback: None,
            narration: None,
            trace_id: None,
            acp_dag_semantic: None,
            bpmn_form: None,
        };
        let chat = repl_to_chat_response(resp, Uuid::nil());
        let decision = chat.decision.expect("constellation map decision");
        assert!(matches!(decision.kind, DecisionKind::ClarifyWorkspace));
        assert_eq!(decision.choices.len(), 1);
        assert_eq!(decision.choices[0].label, "Ireland UCITS ICAV");
        assert!(matches!(chat.session_state, SessionStateEnum::Scoped));
    }

    #[test]
    fn journey_options_maps_to_decision() {
        let resp = ReplResponseV2 {
            state: ReplStateV2::JourneySelection {
                candidates: Some(vec![]),
            },
            kind: ReplResponseKindV2::JourneyOptions {
                packs: vec![PackCandidate {
                    pack_id: "kyc-case".to_string(),
                    pack_name: "KYC Case Management".to_string(),
                    description: "Open and manage KYC cases".to_string(),
                    score: 0.0,
                }],
            },
            message: "Which journey?".to_string(),
            runbook_summary: None,
            step_count: 0,
            session_feedback: None,
            narration: None,
            trace_id: None,
            acp_dag_semantic: None,
            bpmn_form: None,
        };
        let chat = repl_to_chat_response(resp, Uuid::nil());
        let d = chat.decision.unwrap();
        assert!(matches!(d.kind, DecisionKind::ClarifyJourney));
        assert_eq!(d.choices.len(), 1);
    }

    #[test]
    fn sentence_playback_maps_to_coder_proposal() {
        let resp = ReplResponseV2 {
            state: ReplStateV2::SentencePlayback {
                sentence: "Create Allianz Lux CBU".to_string(),
                verb: "cbu.create".to_string(),
                dsl: "(cbu.create :name \"Allianz Lux\")".to_string(),
                args: Default::default(),
            },
            kind: ReplResponseKindV2::SentencePlayback {
                sentence: "Create Allianz Lux CBU".to_string(),
                verb: "cbu.create".to_string(),
                step_sequence: 1,
            },
            message: "Confirm: Create Allianz Lux CBU".to_string(),
            runbook_summary: None,
            step_count: 1,
            session_feedback: None,
            narration: None,
            trace_id: None,
            acp_dag_semantic: None,
            bpmn_form: None,
        };
        let chat = repl_to_chat_response(resp, Uuid::nil());
        assert!(chat.drafter_proposal.is_some());
        let cp = chat.drafter_proposal.unwrap();
        assert_eq!(cp.verb_fqn.unwrap(), "cbu.create");
        assert!(cp.requires_confirmation);
    }

    #[test]
    fn error_maps_to_message() {
        let resp = ReplResponseV2 {
            state: ReplStateV2::RunbookEditing,
            kind: ReplResponseKindV2::Error {
                error: "Verb not found".to_string(),
                recoverable: true,
            },
            message: "Could not find a matching verb.".to_string(),
            runbook_summary: None,
            step_count: 0,
            session_feedback: None,
            narration: None,
            trace_id: None,
            acp_dag_semantic: None,
            bpmn_form: None,
        };
        let chat = repl_to_chat_response(resp, Uuid::nil());
        assert!(chat.message.contains("Could not find"));
        assert!(chat.decision.is_none());
    }
}
