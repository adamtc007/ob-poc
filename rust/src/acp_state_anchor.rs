//! ACP state-anchor provider routing — Repl-coupled drivers.
//!
//! Pure boundary types (descriptors, registries, reports, outcomes,
//! DealTransitionSpec) live in `ob_poc_boundary::acp_state_anchor`
//! and are re-exported here for compat. The async provider drivers
//! below depend on `ReplV2RouteState` (an execution-tier handle) and
//! therefore must stay in this crate.

pub use ob_poc_boundary::acp_state_anchor::*;

use sem_os_policy::state_simulation::{
    SemanticStateDiff, SimulatedStateAdvance, StateSimulationResult,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Instant;
use uuid::Uuid;

use crate::acp_facade::load_ob_poc_kyc_domain_pack;
use crate::api::repl_routes_v2::{handle_repl_acp_request, ReplV2RouteState};

pub(crate) async fn acp_prompt_state_anchor_provider_outcome(
    state: &ReplV2RouteState,
    session_id: Uuid,
    prompt: &[crate::acp_protocol::AcpContentBlock],
    response_id: Value,
) -> AcpPromptStateAnchorProviderOutcome {
    let Some(selection) = acp_prompt_state_anchor_provider_selection(prompt) else {
        return AcpPromptStateAnchorProviderOutcome::continue_without_provider();
    };

    match selection {
        AcpPromptStateAnchorProviderSelection::Provider(
            AcpPromptStateAnchorProvider::KycUpdateStatus,
        ) => {
            acp_prompt_kyc_update_status_state_anchor_provider_outcome(state, session_id, prompt)
                .await
        }
        AcpPromptStateAnchorProviderSelection::Provider(
            AcpPromptStateAnchorProvider::DealUpdateStatus,
        ) => {
            acp_prompt_deal_update_status_state_anchor_provider_outcome(
                state,
                session_id,
                prompt,
                response_id,
            )
            .await
        }
        AcpPromptStateAnchorProviderSelection::UnsupportedStatefulDag => {
            let report = AcpPromptStateAnchorProviderReport::unsupported(vec![
                "supported_sem_os_state_anchor_provider",
                "task_specific_language_pack",
            ]);
            let outgoing = acp_prompt_unsupported_state_anchor_provider_outgoing(
                session_id,
                response_id,
                &report,
            );
            AcpPromptStateAnchorProviderOutcome::Complete { outgoing, report }
        }
    }
}

pub(crate) fn acp_prompt_supported_provider_task(
    prompt: &[crate::acp_protocol::AcpContentBlock],
) -> Option<&'static str> {
    match acp_prompt_state_anchor_provider_selection(prompt) {
        Some(AcpPromptStateAnchorProviderSelection::Provider(provider)) => Some(provider.task()),
        Some(AcpPromptStateAnchorProviderSelection::UnsupportedStatefulDag) | None => None,
    }
}

async fn acp_prompt_kyc_update_status_state_anchor_provider_outcome(
    state: &ReplV2RouteState,
    session_id: Uuid,
    prompt: &[crate::acp_protocol::AcpContentBlock],
) -> AcpPromptStateAnchorProviderOutcome {
    let mut report = AcpPromptStateAnchorProviderReport::for_provider(
        AcpPromptStateAnchorProvider::KycUpdateStatus,
        "selected",
    );

    if acp_prompt_has_read_only_case_state_anchor(prompt) {
        report.status = "prompt_anchor_present";
        report.state_anchor_source = Some("prompt_read_only_discovery_probe");
        report.subject_id = acp_prompt_case_id_from_prompt(prompt);
        return AcpPromptStateAnchorProviderOutcome::Continue {
            outgoing: Vec::new(),
            report,
        };
    }

    let case_id = match acp_prompt_case_id_from_prompt(prompt) {
        Some(case_id) => Some(case_id),
        None => acp_prompt_session_case_id(state, session_id).await,
    };
    let Some(case_id) = case_id else {
        report.status = "missing_subject";
        report.needed = vec!["case_uuid"];
        return AcpPromptStateAnchorProviderOutcome::Continue {
            outgoing: Vec::new(),
            report,
        };
    };
    report.subject_id = Some(case_id);

    let Some(case_state) = acp_prompt_live_case_state(state, session_id, case_id).await else {
        report.status = "state_anchor_unavailable";
        report.needed = vec!["live_case_state"];
        return AcpPromptStateAnchorProviderOutcome::Continue {
            outgoing: Vec::new(),
            report,
        };
    };
    report.status = "seeded";
    report.state_anchor_source = Some("live_read_only_discovery_probe");

    let outgoing = handle_repl_acp_request(
        session_id,
        acp_prompt_live_case_state_discovery_request(session_id, case_state),
    )
    .await;

    AcpPromptStateAnchorProviderOutcome::Continue { outgoing, report }
}

async fn acp_prompt_deal_update_status_state_anchor_provider_outcome(
    state: &ReplV2RouteState,
    session_id: Uuid,
    prompt: &[crate::acp_protocol::AcpContentBlock],
    response_id: Value,
) -> AcpPromptStateAnchorProviderOutcome {
    let started_at = Instant::now();
    let mut report = AcpPromptStateAnchorProviderReport::for_provider(
        AcpPromptStateAnchorProvider::DealUpdateStatus,
        "selected",
    );

    if acp_prompt_has_read_only_deal_state_anchor(prompt) {
        report.status = "prompt_anchor_present";
        report.state_anchor_source = Some("prompt_read_only_discovery_probe");
        report.subject_id = acp_prompt_deal_id_from_prompt(prompt);
        return AcpPromptStateAnchorProviderOutcome::Continue {
            outgoing: Vec::new(),
            report,
        };
    }

    let deal_id = match acp_prompt_deal_id_from_prompt(prompt) {
        Some(deal_id) => Some(deal_id),
        None => acp_prompt_session_deal_id(state, session_id).await,
    };
    let Some(deal_id) = deal_id else {
        report.status = "missing_subject";
        report.needed = vec!["deal_uuid"];
        return AcpPromptStateAnchorProviderOutcome::Continue {
            outgoing: Vec::new(),
            report,
        };
    };
    report.subject_id = Some(deal_id);

    let explicit_current_state = acp_prompt_explicit_current_deal_state(prompt);
    let (current_state, state_anchor_source, configuration_version, state_snapshot_id) =
        match acp_prompt_live_deal_state(state, session_id, deal_id).await {
            Some(live_state) => (
                live_state.current_state,
                "live_read_only_discovery_probe",
                live_state.configuration_version,
                live_state.state_snapshot_id,
            ),
            None => {
                let Some(current_state) = explicit_current_state else {
                    report.status = "state_anchor_unavailable";
                    report.needed = vec!["live_deal_state", "current_state"];
                    let result = acp_state_anchor_pending_question_value(
                        "deal_update_status_state_anchor_missing",
                        vec!["current_state"],
                        elapsed_us(started_at),
                    );
                    let outgoing = acp_state_anchor_structured_outgoing(
                        session_id,
                        response_id,
                        "Deal update-status language loop",
                        result,
                        "I need a read-only deal state anchor before drafting. No dry-run or mutation has run.",
                    );
                    return AcpPromptStateAnchorProviderOutcome::Complete { outgoing, report };
                };
                (
                    current_state.clone(),
                    "prompt_read_only_state_anchor",
                    deal_configuration_version(),
                    format!(
                        "prompt:deal:{deal_id}:deal_status:{}",
                        current_state.to_ascii_lowercase()
                    ),
                )
            }
        };
    report.status = "seeded";
    report.state_anchor_source = Some(state_anchor_source);

    let Some(requested_state) = acp_prompt_requested_deal_state(prompt) else {
        report.status = "missing_requested_state";
        report.needed = vec!["requested_state"];
        let result = acp_state_anchor_pending_question_value(
            "deal_update_status_prompt_incomplete",
            vec!["requested_state"],
            elapsed_us(started_at),
        );
        let outgoing = acp_state_anchor_structured_outgoing(
            session_id,
            response_id,
            "Deal update-status language loop",
            result,
            "I need the requested deal status before drafting a dry-run workbook. No mutation has run.",
        );
        return AcpPromptStateAnchorProviderOutcome::Complete { outgoing, report };
    };

    let language_pack = deal_update_status_language_pack(
        deal_id,
        &current_state,
        configuration_version,
        state_snapshot_id,
        Some(format!(
            "Draft a dry-run workbook for deal.update-status from {current_state} to {requested_state}"
        )),
    );

    let Some(evidence_digest) = acp_prompt_evidence_digest(prompt) else {
        let result = acp_deal_update_status_structured_refusal_value(
            &language_pack,
            "deal_update_status_refused",
            "missing_evidence_digest",
            "workbook.evidence_refs[0].digest",
            &current_state,
            Some(&requested_state),
            Some("deal.update-status requires an evidence digest before dry-run validation"),
            elapsed_us(started_at),
        );
        let outgoing = acp_state_anchor_structured_outgoing(
            session_id,
            response_id,
            "Deal update-status language loop",
            result,
            "I stopped because `deal.update-status` requires an evidence digest before dry-run validation. No mutation has run.",
        );
        return AcpPromptStateAnchorProviderOutcome::Complete { outgoing, report };
    };

    let Some(transition) = deal_update_status_transition(&current_state, &requested_state) else {
        let result = acp_deal_update_status_structured_refusal_value(
            &language_pack,
            "deal_update_status_refused",
            "unknown_transition",
            "workbook.transition_ref",
            &current_state,
            Some(&requested_state),
            Some("requested target state is not reachable by deal.update-status from the current deal state"),
            elapsed_us(started_at),
        );
        let outgoing = acp_state_anchor_structured_outgoing(
            session_id,
            response_id,
            "Deal update-status language loop",
            result,
            "I stopped because the requested deal transition is not valid from the anchored state. No mutation has run.",
        );
        return AcpPromptStateAnchorProviderOutcome::Complete { outgoing, report };
    };

    let result = match build_deal_update_status_dry_run_value(
        session_id,
        &language_pack,
        transition,
        evidence_digest,
        elapsed_us(started_at),
    ) {
        Ok(result) => result,
        Err(result) => result,
    };
    let text = if result.get("status").and_then(Value::as_str) == Some("dry_run_validated") {
        format!(
            "I read the deal state as `{current_state}`, selected `{}` for `{requested_state}`, and validated a dry-run workbook. No mutation has run.",
            transition.transition_ref
        )
    } else {
        "I stopped with a structured deal workbook refusal. No mutation has run.".to_string()
    };
    let outgoing = acp_state_anchor_structured_outgoing(
        session_id,
        response_id,
        "Deal update-status language loop",
        result,
        text,
    );

    AcpPromptStateAnchorProviderOutcome::Complete { outgoing, report }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcpPromptStateAnchorProviderSelection {
    Provider(AcpPromptStateAnchorProvider),
    UnsupportedStatefulDag,
}

fn acp_prompt_state_anchor_provider_selection(
    prompt: &[crate::acp_protocol::AcpContentBlock],
) -> Option<AcpPromptStateAnchorProviderSelection> {
    if acp_prompt_looks_like_kyc_update_status(prompt) {
        return Some(AcpPromptStateAnchorProviderSelection::Provider(
            AcpPromptStateAnchorProvider::KycUpdateStatus,
        ));
    }
    if acp_prompt_looks_like_deal_update_status(prompt) {
        return Some(AcpPromptStateAnchorProviderSelection::Provider(
            AcpPromptStateAnchorProvider::DealUpdateStatus,
        ));
    }
    if acp_prompt_looks_like_stateful_transition_request(prompt) {
        return Some(AcpPromptStateAnchorProviderSelection::UnsupportedStatefulDag);
    }
    None
}

fn acp_prompt_unsupported_state_anchor_provider_outgoing(
    session_id: Uuid,
    response_id: Value,
    report: &AcpPromptStateAnchorProviderReport,
) -> Vec<crate::acp_protocol::JsonRpcOutgoing> {
    let supported = report.supported_tasks.join(", ");
    let message = format!(
        "I can see a state-transition style request, but this SemOS state-anchor provider is not wired yet. I need a supported task-specific provider and language pack before drafting a workbook. Supported now: {supported}. No dry-run or mutation has run."
    );
    let mut result = json!({
        "stopReason": "end_turn",
        "status": "pending_question",
        "pending_question": {
            "code": "sem_os_state_anchor_provider_unavailable",
            "needs": report.needed,
            "supported_tasks": report.supported_tasks
        },
        "metrics": {
            "language_pack_generated": false,
            "invented_verb_count": 0,
            "uuid_binding_complete": false,
            "state_valid_transition_selected": false,
            "first_pass_valid": false,
            "revision_count": 0,
            "dry_run_valid": false,
            "refusal_code": "sem_os_state_anchor_provider_unavailable"
        },
        "observability": {
            "performance": zero_performance(),
            "conversationEfficiency": {
                "outcome": "pending_question",
                "localRevisionCount": 0,
                "estimatedUserRepairTurnsAvoided": 0,
                "pendingUserTurnRequired": true,
                "pendingReason": "sem_os_state_anchor_provider_unavailable",
                "firstPassValid": false,
                "dryRunValid": false,
                "structuredFailureMode": "sem_os_state_anchor_provider_unavailable",
                "proseOnlyFailure": false
            },
            "acpMechanismSummary": [
                "state_anchor_provider_router",
                "structured_pending_question",
                "dry_run_only"
            ]
        },
        "trace": [
            {
                "phase": "state_anchor_provider",
                "status": "blocked",
                "message": "No supported SemOS state-anchor provider is wired for this task"
            }
        ]
    });
    let state_anchor_provider = report.metrics(Some(&result));
    if let Some(observability) = result
        .get_mut("observability")
        .and_then(|value| value.as_object_mut())
    {
        observability.insert("stateAnchorProvider".to_string(), state_anchor_provider);
    }

    vec![
        crate::acp_protocol::JsonRpcOutgoing::Notification(
            crate::acp_protocol::JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "session/update".to_string(),
                params: json!({
                    "sessionId": session_id.to_string(),
                    "update": {
                        "sessionUpdate": "plan",
                        "persona": crate::acp::AcpPersonaMode::SagePlanning.as_str(),
                        "workflowPhase": "discovery",
                        "goalProposalTrace": result,
                        "entries": [
                            {"id": "state-anchor-provider", "status": "blocked", "label": "Select SemOS state-anchor provider"},
                            {"id": "language-pack", "status": "blocked", "label": "Retrieve task language pack"},
                            {"id": "dry-run", "status": "blocked", "label": "Await supported provider"}
                        ]
                    }
                }),
            },
        ),
        crate::acp_protocol::JsonRpcOutgoing::Notification(
            crate::acp_protocol::JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "session/update".to_string(),
                params: json!({
                    "sessionId": session_id.to_string(),
                    "update": {
                        "sessionUpdate": "agent_message_chunk",
                        "content": {"type": "text", "text": message}
                    }
                }),
            },
        ),
        crate::acp_protocol::JsonRpcOutgoing::Response(crate::acp_protocol::JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(response_id),
            result: Some(result),
            error: None,
        }),
    ]
}

pub(crate) async fn acp_prompt_session_case_id(
    state: &ReplV2RouteState,
    session_id: Uuid,
) -> Option<Uuid> {
    let session = state.orchestrator.get_session(session_id).await?;
    session.workspace_stack.iter().rev().find_map(|frame| {
        frame.current_case_id.or_else(|| {
            if frame.subject_kind == Some(crate::repl::types_v2::SubjectKind::Case) {
                frame.subject_id
            } else {
                None
            }
        })
    })
}

async fn acp_prompt_session_deal_id(state: &ReplV2RouteState, session_id: Uuid) -> Option<Uuid> {
    let session = state.orchestrator.get_session(session_id).await?;
    session.workspace_stack.iter().rev().find_map(|frame| {
        frame.deal_id.or_else(|| {
            if frame.subject_kind == Some(crate::repl::types_v2::SubjectKind::Deal) {
                frame.subject_id
            } else {
                None
            }
        })
    })
}

#[derive(Debug, Clone)]
struct AcpPromptLiveCaseState {
    subject_id: Uuid,
    current_state: String,
    configuration_version: String,
    state_snapshot_id: String,
}

#[derive(Debug, Clone)]
struct AcpPromptLiveDealState {
    current_state: String,
    configuration_version: String,
    state_snapshot_id: String,
}

#[cfg(feature = "database")]
async fn acp_prompt_live_case_state(
    state: &ReplV2RouteState,
    session_id: Uuid,
    case_id: Uuid,
) -> Option<AcpPromptLiveCaseState> {
    let pool = state.orchestrator.pool()?;
    let row = match sqlx::query_as::<_, (Option<String>,)>(
        r#"SELECT status FROM "ob-poc".cases WHERE case_id = $1"#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await
    {
        Ok(row) => row,
        Err(error) => {
            tracing::warn!(
                session_id = %session_id,
                case_id = %case_id,
                error = %error,
                "Failed to read live KYC case state for ACP prompt"
            );
            return None;
        }
    };
    let (status,) = row?;
    let current_state = status
        .filter(|status| !status.trim().is_empty())
        .unwrap_or_else(|| "INTAKE".to_string())
        .trim()
        .to_ascii_uppercase();
    let configuration_version = load_ob_poc_kyc_domain_pack()
        .map(|manifest| format!("domain_pack:{}@{}", manifest.pack_id, manifest.version))
        .unwrap_or_else(|_| "domain_pack:ob-poc.kyc".to_string());
    let state_snapshot_id = format!(
        "postgres:ob-poc.cases:{case_id}:status:{}",
        current_state.to_ascii_lowercase()
    );

    Some(AcpPromptLiveCaseState {
        subject_id: case_id,
        current_state,
        configuration_version,
        state_snapshot_id,
    })
}

#[cfg(not(feature = "database"))]
async fn acp_prompt_live_case_state(
    _state: &ReplV2RouteState,
    _session_id: Uuid,
    _case_id: Uuid,
) -> Option<AcpPromptLiveCaseState> {
    None
}

#[cfg(feature = "database")]
async fn acp_prompt_live_deal_state(
    state: &ReplV2RouteState,
    session_id: Uuid,
    deal_id: Uuid,
) -> Option<AcpPromptLiveDealState> {
    let pool = state.orchestrator.pool()?;
    let row = match sqlx::query_as::<_, (Option<String>,)>(
        r#"SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1"#,
    )
    .bind(deal_id)
    .fetch_optional(pool)
    .await
    {
        Ok(row) => row,
        Err(error) => {
            tracing::warn!(
                session_id = %session_id,
                deal_id = %deal_id,
                error = %error,
                "Failed to read live deal state for ACP prompt"
            );
            return None;
        }
    };
    let (status,) = row?;
    let current_state = status
        .filter(|status| !status.trim().is_empty())
        .map(|status| normalize_deal_state(&status))
        .unwrap_or_else(|| "PROSPECT".to_string());
    let state_snapshot_id = format!(
        "postgres:ob-poc.deals:{deal_id}:deal_status:{}",
        current_state.to_ascii_lowercase()
    );

    Some(AcpPromptLiveDealState {
        current_state,
        configuration_version: deal_configuration_version(),
        state_snapshot_id,
    })
}

#[cfg(not(feature = "database"))]
async fn acp_prompt_live_deal_state(
    _state: &ReplV2RouteState,
    _session_id: Uuid,
    _deal_id: Uuid,
) -> Option<AcpPromptLiveDealState> {
    None
}

fn acp_prompt_live_case_state_discovery_request(
    session_id: Uuid,
    case_state: AcpPromptLiveCaseState,
) -> crate::acp_protocol::JsonRpcRequest {
    crate::acp_protocol::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!("live-case-state-discovery")),
        method: "obpoc/kyc_case_state/discover".to_string(),
        params: json!({
            "session_id": session_id,
            "sessionId": session_id,
            "adapter": crate::acp::AcpAdapterKind::Zed,
            "subject_id": case_state.subject_id,
            "observations": [
                {"key": "case.status", "value": case_state.current_state, "classification": "internal"},
                {"key": "case.configuration_version", "value": case_state.configuration_version, "classification": "internal"},
                {"key": "case.state_snapshot_id", "value": case_state.state_snapshot_id, "classification": "internal"}
            ],
            "provenance": [
                {"source": "postgres.ob-poc.cases", "snapshot_ref": case_state.state_snapshot_id}
            ],
            "first_class_state_mutated": false
        }),
    }
}

pub(crate) fn acp_prompt_blocks_from_params(
    params: &Value,
) -> Option<Vec<crate::acp_protocol::AcpContentBlock>> {
    serde_json::from_value::<crate::acp_protocol::AcpPromptRequest>(params.clone())
        .ok()
        .map(|request| request.prompt)
}

/// Helper to check if `text` contains `word` as a standalone word.
///
/// A word boundary is defined by the start/end of the string or non-alphanumeric characters.
fn contains_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let word_bytes = word.as_bytes();
    let first_is_alphanumeric = word_bytes[0].is_ascii_alphanumeric();
    let last_is_alphanumeric = word_bytes[word_bytes.len() - 1].is_ascii_alphanumeric();

    let mut start = 0;
    while let Some(pos) = text[start..].find(word) {
        let abs_pos = start + pos;

        let before_ok = if first_is_alphanumeric {
            abs_pos == 0 || !text.as_bytes()[abs_pos - 1].is_ascii_alphanumeric()
        } else {
            true
        };

        let end_pos = abs_pos + word.len();
        let after_ok = if last_is_alphanumeric {
            end_pos == text.len() || !text.as_bytes()[end_pos].is_ascii_alphanumeric()
        } else {
            true
        };

        if before_ok && after_ok {
            return true;
        }
        start = abs_pos + 1;
    }
    false
}

fn acp_prompt_looks_like_kyc_update_status(
    prompt: &[crate::acp_protocol::AcpContentBlock],
) -> bool {
    let lower = acp_prompt_text(prompt).to_ascii_lowercase();
    (contains_word(&lower, "kyc") || lower.contains("kyc-case.update-status"))
        && acp_prompt_has_state_transition_intent(&lower)
}

fn acp_prompt_looks_like_deal_update_status(
    prompt: &[crate::acp_protocol::AcpContentBlock],
) -> bool {
    let lower = acp_prompt_text(prompt).to_ascii_lowercase();
    (contains_word(&lower, "deal") || lower.contains(DEAL_UPDATE_STATUS_TASK))
        && acp_prompt_has_state_transition_intent(&lower)
}

fn acp_prompt_looks_like_stateful_transition_request(
    prompt: &[crate::acp_protocol::AcpContentBlock],
) -> bool {
    let lower = acp_prompt_text(prompt).to_ascii_lowercase();
    let has_state_subject = contains_word(&lower, "case")
        || contains_word(&lower, "workflow")
        || contains_word(&lower, "dag")
        || contains_word(&lower, "state")
        || contains_word(&lower, "status")
        || contains_word(&lower, "deal");
    has_state_subject && acp_prompt_has_state_transition_intent(&lower)
}

fn acp_prompt_has_state_transition_intent(lower: &str) -> bool {
    contains_word(lower, "update-status")
        || contains_word(lower, "update status")
        || contains_word(lower, "advance")
        || contains_word(lower, "transition")
        || contains_word(lower, "move")
        || contains_word(lower, "change status")
        || contains_word(lower, "set status")
}

fn acp_prompt_has_read_only_case_state_anchor(
    prompt: &[crate::acp_protocol::AcpContentBlock],
) -> bool {
    prompt.iter().any(|block| match block {
        crate::acp_protocol::AcpContentBlock::EmbeddedResource { text, .. } => text
            .as_deref()
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .map(|value| {
                value
                    .get("probe_id")
                    .or_else(|| value.get("probeId"))
                    .and_then(|probe_id| probe_id.as_str())
                    == Some("kyc-case.read-state")
            })
            .unwrap_or(false),
        crate::acp_protocol::AcpContentBlock::Text { .. }
        | crate::acp_protocol::AcpContentBlock::ResourceLink { .. } => false,
    })
}

fn acp_prompt_has_read_only_deal_state_anchor(
    prompt: &[crate::acp_protocol::AcpContentBlock],
) -> bool {
    prompt.iter().any(|block| match block {
        crate::acp_protocol::AcpContentBlock::EmbeddedResource { text, .. } => text
            .as_deref()
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .map(|value| {
                value
                    .get("probe_id")
                    .or_else(|| value.get("probeId"))
                    .and_then(|probe_id| probe_id.as_str())
                    == Some("deal.read-state")
            })
            .unwrap_or(false),
        crate::acp_protocol::AcpContentBlock::Text { .. }
        | crate::acp_protocol::AcpContentBlock::ResourceLink { .. } => false,
    })
}

fn acp_prompt_case_id_from_prompt(prompt: &[crate::acp_protocol::AcpContentBlock]) -> Option<Uuid> {
    acp_subject_id_from_prompt(prompt)
}

fn acp_prompt_deal_id_from_prompt(prompt: &[crate::acp_protocol::AcpContentBlock]) -> Option<Uuid> {
    acp_subject_id_from_prompt(prompt)
}

fn acp_subject_id_from_prompt(prompt: &[crate::acp_protocol::AcpContentBlock]) -> Option<Uuid> {
    prompt.iter().find_map(|block| match block {
        crate::acp_protocol::AcpContentBlock::Text { text } => acp_extract_first_uuid(text),
        crate::acp_protocol::AcpContentBlock::ResourceLink { uri, .. } => {
            acp_entity_uuid_from_uri(uri).or_else(|| acp_extract_first_uuid(uri))
        }
        crate::acp_protocol::AcpContentBlock::EmbeddedResource { uri, text, .. } => text
            .as_deref()
            .and_then(acp_subject_id_from_embedded_resource_text)
            .or_else(|| acp_entity_uuid_from_uri(uri))
            .or_else(|| acp_extract_first_uuid(uri)),
    })
}

fn acp_subject_id_from_embedded_resource_text(text: &str) -> Option<Uuid> {
    let value = serde_json::from_str::<Value>(text).ok()?;
    value
        .pointer("/subject/subject_id")
        .or_else(|| value.pointer("/subject/subjectId"))
        .or_else(|| value.pointer("/case_state/subject_id"))
        .or_else(|| value.pointer("/caseState/subjectId"))
        .or_else(|| value.pointer("/deal_state/subject_id"))
        .or_else(|| value.pointer("/dealState/subjectId"))
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn acp_entity_uuid_from_uri(uri: &str) -> Option<Uuid> {
    let rest = uri.strip_prefix("semos://entity/")?;
    let id = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    Uuid::parse_str(id).ok()
}

fn acp_extract_first_uuid(text: &str) -> Option<Uuid> {
    text.split(|ch: char| !(ch.is_ascii_hexdigit() || ch == '-'))
        .find_map(|token| Uuid::parse_str(token).ok())
}

fn acp_prompt_text(prompt: &[crate::acp_protocol::AcpContentBlock]) -> String {
    prompt
        .iter()
        .filter_map(|block| match block {
            crate::acp_protocol::AcpContentBlock::Text { text } => Some(text.as_str()),
            crate::acp_protocol::AcpContentBlock::ResourceLink { .. }
            | crate::acp_protocol::AcpContentBlock::EmbeddedResource { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

const DEAL_STATES: &[&str] = &[
    "PROSPECT",
    "QUALIFYING",
    "NEGOTIATING",
    "IN_CLEARANCE",
    "CONTRACTED",
    "LOST",
    "REJECTED",
    "WITHDRAWN",
    "CANCELLED",
];

fn deal_update_status_transition(
    current_state: &str,
    requested_state: &str,
) -> Option<&'static DealTransitionSpec> {
    let current_state = normalize_deal_state(current_state);
    let requested_state = normalize_deal_state(requested_state);
    DEAL_UPDATE_STATUS_TRANSITIONS.iter().find(|transition| {
        transition.from_state == current_state && transition.to_state == requested_state
    })
}

fn acp_prompt_explicit_current_deal_state(
    prompt: &[crate::acp_protocol::AcpContentBlock],
) -> Option<String> {
    let text = normalize_prompt_for_state_matching(&acp_prompt_text(prompt));
    state_after_marker(&text, "from")
        .or_else(|| state_after_marker(&text, "current state"))
        .or_else(|| state_after_marker(&text, "current status"))
}

fn acp_prompt_requested_deal_state(
    prompt: &[crate::acp_protocol::AcpContentBlock],
) -> Option<String> {
    let text = normalize_prompt_for_state_matching(&acp_prompt_text(prompt));
    state_after_marker(&text, "to")
        .or_else(|| state_after_marker(&text, "target state"))
        .or_else(|| state_after_marker(&text, "requested state"))
        .or_else(|| state_after_marker(&text, "requested status"))
}

fn state_after_marker(text: &str, marker: &str) -> Option<String> {
    DEAL_STATES.iter().find_map(|state| {
        let normalized = state.to_ascii_lowercase().replace('_', " ");
        let pattern = format!("{marker} {normalized}");
        if text.contains(&pattern) {
            Some((*state).to_string())
        } else {
            None
        }
    })
}

fn normalize_prompt_for_state_matching(text: &str) -> String {
    text.to_ascii_lowercase()
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_deal_state(value: &str) -> String {
    value.trim().replace(['-', ' '], "_").to_ascii_uppercase()
}

fn acp_prompt_evidence_digest(prompt: &[crate::acp_protocol::AcpContentBlock]) -> Option<String> {
    let text = acp_prompt_text(prompt);
    let start = text.find("sha256:")?;
    let digest = text[start..]
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ')' | ']' | '}'))
        .next()
        .unwrap_or_default()
        .trim_matches(|ch| matches!(ch, '.' | ';' | ':' | ','));
    if digest == "sha256" || digest == "sha256:" || digest.is_empty() {
        None
    } else {
        Some(digest.to_string())
    }
}

fn deal_configuration_version() -> String {
    format!("state_machine:{DEAL_STATE_MACHINE}@{DEAL_PACK_VERSION}")
}

fn deal_update_status_language_pack(
    deal_id: Uuid,
    current_state: &str,
    configuration_version: String,
    state_snapshot_id: String,
    objective: Option<String>,
) -> crate::runbook::SemOsLanguagePack {
    let candidate_transitions = DEAL_UPDATE_STATUS_TRANSITIONS
        .iter()
        .filter(|transition| transition.from_state == current_state)
        .map(deal_language_transition)
        .collect::<Vec<_>>();
    let blocked_verbs = DEAL_UPDATE_STATUS_TRANSITIONS
        .iter()
        .filter(|transition| transition.from_state != current_state)
        .map(|transition| crate::runbook::BlockedVerb {
            verb: DEAL_UPDATE_STATUS_TASK.to_string(),
            reason: format!(
                "{} is blocked because current state is {}, not {}",
                transition.transition_ref, current_state, transition.from_state
            ),
        })
        .collect::<Vec<_>>();
    let transition_effects = candidate_transitions
        .iter()
        .map(|transition| crate::runbook::TransitionEffect {
            transition_ref: transition.transition_ref.clone(),
            field: "deal_status".to_string(),
            before: transition.from_state.clone(),
            after: transition.to_state.clone(),
            writes_since_push_delta: 1,
        })
        .collect::<Vec<_>>();

    crate::runbook::SemOsLanguagePack {
        objective: objective.unwrap_or_else(|| {
            format!(
                "Draft a dry-run workbook for deal.update-status from {current_state}"
            )
        }),
        pack_id: DEAL_PACK_ID.to_string(),
        pack_version: DEAL_PACK_VERSION.to_string(),
        configuration_version,
        state_snapshot_id,
        subject: crate::runbook::LanguagePackSubject {
            kind: "deal".to_string(),
            id: deal_id,
        },
        current_state: current_state.to_string(),
        candidate_transitions,
        valid_verbs: vec![crate::runbook::LanguagePackVerb {
            verb: DEAL_UPDATE_STATUS_TASK.to_string(),
            reason: "Only deal status transition workbook drafting is in scope".to_string(),
        }],
        blocked_verbs,
        argument_schema: deal_argument_schema(),
        transition_effects,
        evidence_policy: crate::runbook::EvidencePolicySummary {
            required_evidence_refs: vec!["deal_id".to_string()],
            dry_run_only: true,
            mutation_allowed: false,
            hitl_required: true,
        },
        uuid_bindings: vec![crate::runbook::UuidBindingRequirement {
            field: "deal_id".to_string(),
            subject_kind: "deal".to_string(),
            required: true,
            expected_uuid: deal_id,
        }],
        canonical_patterns: vec![
            pattern(
                "happy_path",
                "Use verb deal.update-status with the candidate transition whose from_state equals current_state.",
            ),
            pattern(
                "uuid_binding",
                "Bind deal_id to the active deal UUID from uuid_bindings; do not invent a UUID.",
            ),
            pattern(
                "state_binding",
                "Set current_state to the observed language-pack current_state.",
            ),
            pattern(
                "target_binding",
                "Set requested_state to the selected transition to_state.",
            ),
            pattern(
                "dry_run_only",
                "Produce a dry-run workbook only; ACP mutation and direct execution are out of scope.",
            ),
        ],
    }
}

fn deal_language_transition(
    transition: &DealTransitionSpec,
) -> crate::runbook::LanguagePackTransition {
    crate::runbook::LanguagePackTransition {
        transition_ref: transition.transition_ref.to_string(),
        verb: DEAL_UPDATE_STATUS_TASK.to_string(),
        from_state: transition.from_state.to_string(),
        to_state: transition.to_state.to_string(),
        dry_run_enabled: true,
        mutation_enabled: false,
        hitl_required: true,
        evidence_refs_required: vec!["deal_id".to_string()],
    }
}

fn deal_argument_schema() -> Vec<crate::runbook::LanguagePackArg> {
    vec![
        arg("deal_id", "uuid", "active deal subject UUID"),
        arg(
            "transition_ref",
            "string",
            "declared SemOS deal transition_ref",
        ),
        arg("current_state", "enum", "observed current deal_status"),
        arg("requested_state", "enum", "requested target deal_status"),
        arg(
            "configuration_version",
            "string",
            "state machine/config anchor",
        ),
        arg("state_snapshot_id", "string", "state snapshot anchor"),
        arg(
            "evidence_digest",
            "string",
            "digest for required deal evidence",
        ),
    ]
}

fn arg(name: &str, arg_type: &str, binding: &str) -> crate::runbook::LanguagePackArg {
    crate::runbook::LanguagePackArg {
        name: name.to_string(),
        arg_type: arg_type.to_string(),
        required: true,
        binding: binding.to_string(),
    }
}

fn pattern(name: &str, draft_shape: &str) -> crate::runbook::CanonicalMicroPattern {
    crate::runbook::CanonicalMicroPattern {
        name: name.to_string(),
        draft_shape: draft_shape.to_string(),
    }
}

fn build_deal_update_status_dry_run_value(
    session_id: Uuid,
    language_pack: &crate::runbook::SemOsLanguagePack,
    transition: &DealTransitionSpec,
    evidence_digest: String,
    total_us: u64,
) -> Result<Value, Value> {
    let deal_id = language_pack.subject.id;
    let current_state = language_pack.current_state.clone();
    let requested_state = transition.to_state.to_string();
    let simulation = StateSimulationResult {
        transition_ref: transition.transition_ref.to_string(),
        entity_id: deal_id,
        entity_type: "deal".to_string(),
        state_machine: DEAL_STATE_MACHINE.to_string(),
        from_state: current_state.clone(),
        to_state: requested_state.clone(),
        verb: DEAL_UPDATE_STATUS_TASK.to_string(),
        semantic_diff: SemanticStateDiff {
            field: "deal_status".to_string(),
            before: current_state.clone(),
            after: requested_state.clone(),
        },
        predicted_advance: SimulatedStateAdvance {
            entity_id: deal_id,
            to_node: format!("deal:{}", requested_state.to_ascii_lowercase()),
            slot_path: "deal-lifecycle/deal".to_string(),
            reason: format!("{DEAL_UPDATE_STATUS_TASK} - {current_state} -> {requested_state}"),
            writes_since_push_delta: 1,
        },
        state_snapshot_id: Some(language_pack.state_snapshot_id.clone()),
        configuration_version: Some(language_pack.configuration_version.clone()),
    };
    let core = crate::runbook::workbook::ExecutionWorkbookCore {
        schema_version: 1,
        pack_id: language_pack.pack_id.clone(),
        transition_ref: transition.transition_ref.to_string(),
        execution_mode: crate::runbook::workbook::WorkbookExecutionMode::DryRun,
        session_id,
        subject: crate::runbook::workbook::WorkbookSubject {
            subject_kind: "deal".to_string(),
            subject_id: deal_id,
        },
        actor: crate::runbook::workbook::WorkbookActor {
            actor_id: "sage:planning".to_string(),
            roles: vec!["agent".to_string()],
        },
        configuration_version: language_pack.configuration_version.clone(),
        state_snapshot_id: language_pack.state_snapshot_id.clone(),
        objective: language_pack.objective.clone(),
        user_prompt_ref: None,
        editor_context_refs: vec![format!("semos://entity/{deal_id}")],
        evidence_refs: vec![crate::runbook::workbook::EvidenceRef {
            kind: "deal_id".to_string(),
            ref_id: deal_id.to_string(),
            digest: evidence_digest,
            source_system: Some("ob-poc".to_string()),
            field_path: Some("deals.deal_id".to_string()),
            classification: Some("internal".to_string()),
        }],
        llm_trace_ref: None,
        expected_preconditions: vec![format!("deal_status == {current_state}")],
        expected_postconditions: vec![format!("deal_status == {requested_state}")],
        invariant_checks: vec![crate::runbook::workbook::WorkbookCheck {
            check_id: "deal.transition.frontier".to_string(),
            status: crate::runbook::workbook::WorkbookCheckStatus::Passed,
            message: "transition is declared in the bounded deal provider registry".to_string(),
        }],
        governance_checks: vec![crate::runbook::workbook::WorkbookCheck {
            check_id: "deal.evidence.deal_id".to_string(),
            status: crate::runbook::workbook::WorkbookCheckStatus::Passed,
            message: "deal evidence reference present".to_string(),
        }],
        simulation,
        stale_policy: crate::runbook::workbook::StaleWorkbookPolicy::Revalidate,
        previous_workbook_id: None,
        metadata: BTreeMap::from([
            (
                "provider_id".to_string(),
                "deal.update_status.live_deal_state".to_string(),
            ),
            ("dry_run_only".to_string(), "true".to_string()),
        ]),
    };
    let workbook = match crate::runbook::workbook::ExecutionWorkbook::new(core) {
        Ok(workbook) => workbook,
        Err(error) => {
            return Err(acp_deal_update_status_structured_refusal_value(
                language_pack,
                "deal_update_status_refused",
                "workbook_integrity_failed",
                "workbook",
                &current_state,
                Some(&requested_state),
                Some(format!("{error:?}").as_str()),
                total_us,
            ));
        }
    };
    let dry_run = match crate::runbook::validate_workbook_for_dry_run(
        &workbook,
        crate::runbook::DslDrafterExecutionMode::DryRun,
    ) {
        Ok(dry_run) => dry_run,
        Err(error) => {
            return Err(acp_deal_update_status_structured_refusal_value(
                language_pack,
                "deal_update_status_refused",
                "workbook_validation_failed",
                "workbook",
                &current_state,
                Some(&requested_state),
                Some(format!("{error:?}").as_str()),
                total_us,
            ));
        }
    };
    let trace_projection = deal_trace_projection(
        "dry_run_validated",
        language_pack,
        Some(&dry_run),
        Some(transition.transition_ref),
        Some(&requested_state),
        None,
        vec![],
        vec![],
    );
    Ok(json!({
        "status": "dry_run_validated",
        "draft_source": "deterministic_provider",
        "prompt_context_variant": {"id": "deterministic_language_loop"},
        "language_pack": language_pack,
        "output": {
            "workbook": workbook,
            "dry_run": dry_run
        },
        "attempts": [],
        "metrics": {
            "language_pack_generated": true,
            "invented_verb_count": 0,
            "uuid_binding_complete": true,
            "state_valid_transition_selected": true,
            "first_pass_valid": true,
            "revision_count": 0,
            "dry_run_valid": true,
            "refusal_code": Value::Null
        },
        "trace": [
            {"phase": "language_pack", "status": "completed"},
            {"phase": "draft", "status": "completed"},
            {"phase": "validate", "status": "completed"},
            {"phase": "dry_run", "status": "completed"}
        ],
        "traceProjection": trace_projection,
        "observability": {
            "projectionLatencyMs": millis_from_micros(total_us),
            "performance": performance(total_us),
            "conversationEfficiency": {
                "outcome": "dry_run_validated",
                "localRevisionCount": 0,
                "estimatedUserRepairTurnsAvoided": 1,
                "pendingUserTurnRequired": false,
                "pendingReason": Value::Null,
                "firstPassValid": true,
                "dryRunValid": true,
                "structuredFailureMode": Value::Null,
                "proseOnlyFailure": false
            },
            "acpMechanismSummary": ["state_anchor_provider_router", "language_pack", "deterministic_revision_loop", "dry_run_only"]
        }
    }))
}

#[allow(clippy::too_many_arguments)]
fn acp_deal_update_status_structured_refusal_value(
    language_pack: &crate::runbook::SemOsLanguagePack,
    refusal_code: &str,
    diagnostic_code: &str,
    source_path: &str,
    current_state: &str,
    requested_state: Option<&str>,
    message: Option<&str>,
    total_us: u64,
) -> Value {
    let needs = match diagnostic_code {
        "missing_evidence_digest" => vec!["evidence_digest"],
        "unknown_transition" => vec!["requested_state"],
        _ => vec!["corrected_workbook_draft"],
    };
    let diagnostic = json!({
        "error_code": diagnostic_code,
        "attempted_transition": requested_state.and_then(|requested| {
            deal_update_status_transition(current_state, requested)
                .map(|transition| transition.transition_ref)
        }),
        "attempted_verb": DEAL_UPDATE_STATUS_TASK,
        "source_path": source_path,
        "source_step": "provider.validation",
        "expected_state": language_pack.candidate_transitions.first().map(|transition| transition.to_state.as_str()),
        "actual_state": requested_state,
        "missing_uuid_binding": Value::Null,
        "blocked_transition_reason": message,
        "suggested_valid_transitions": language_pack
            .candidate_transitions
            .iter()
            .map(|transition| transition.transition_ref.clone())
            .collect::<Vec<_>>(),
        "suggested_valid_verbs": [DEAL_UPDATE_STATUS_TASK],
        "pack_ref": format!("{}@{}", language_pack.pack_id, language_pack.pack_version),
        "configuration_version": language_pack.configuration_version,
        "state_snapshot_id": language_pack.state_snapshot_id
    });
    let trace_projection = deal_trace_projection(
        "structured_refusal",
        language_pack,
        None,
        requested_state.and_then(|requested| {
            deal_update_status_transition(current_state, requested)
                .map(|transition| transition.transition_ref)
        }),
        requested_state,
        Some(refusal_code),
        vec![diagnostic_code.to_string()],
        needs.iter().map(|need| (*need).to_string()).collect(),
    );
    json!({
        "status": "structured_refusal",
        "draft_source": "deterministic_provider",
        "prompt_context_variant": {"id": "deterministic_language_loop"},
        "language_pack": language_pack,
        "refusal": {
            "refusal_code": refusal_code,
            "message": message.unwrap_or("deal.update-status workbook validation refused"),
            "diagnostics": [diagnostic]
        },
        "attempts": [],
        "metrics": {
            "language_pack_generated": true,
            "invented_verb_count": 0,
            "uuid_binding_complete": true,
            "state_valid_transition_selected": diagnostic_code != "unknown_transition",
            "first_pass_valid": false,
            "revision_count": 0,
            "dry_run_valid": false,
            "refusal_code": refusal_code
        },
        "trace": [
            {"phase": "language_pack", "status": "completed"},
            {"phase": "validate", "status": "refused", "diagnostic_code": diagnostic_code}
        ],
        "traceProjection": trace_projection,
        "observability": {
            "projectionLatencyMs": millis_from_micros(total_us),
            "performance": performance(total_us),
            "conversationEfficiency": {
                "outcome": "structured_refusal",
                "localRevisionCount": 0,
                "estimatedUserRepairTurnsAvoided": 1,
                "pendingUserTurnRequired": false,
                "pendingReason": Value::Null,
                "firstPassValid": false,
                "dryRunValid": false,
                "structuredFailureMode": refusal_code,
                "proseOnlyFailure": false
            },
            "acpMechanismSummary": ["state_anchor_provider_router", "language_pack", "structured_refusal", "dry_run_only"]
        }
    })
}

fn acp_state_anchor_pending_question_value(code: &str, needs: Vec<&str>, total_us: u64) -> Value {
    json!({
        "stopReason": "end_turn",
        "status": "pending_question",
        "pending_question": {
            "code": code,
            "needs": needs,
            "supported_tasks": supported_tasks()
        },
        "metrics": {
            "language_pack_generated": false,
            "invented_verb_count": 0,
            "uuid_binding_complete": false,
            "state_valid_transition_selected": false,
            "first_pass_valid": false,
            "revision_count": 0,
            "dry_run_valid": false,
            "refusal_code": code
        },
        "traceProjection": {
            "outcome": "pending_question",
            "promptContextVariant": "deterministic_language_loop",
            "outcomeLayer": "pre_llm_pending",
            "diagnosticCodes": [],
            "revisionCount": 0,
            "decodeRepairCount": 0,
            "firstPassValid": false,
            "dryRunValid": false,
            "humanSummary": "I stopped before drafting because a state anchor or UUID binding is missing; HITL clarification is needed.",
            "neededFromUser": needs
        },
        "observability": {
            "projectionLatencyMs": millis_from_micros(total_us),
            "performance": performance(total_us),
            "conversationEfficiency": {
                "outcome": "pending_question",
                "localRevisionCount": 0,
                "estimatedUserRepairTurnsAvoided": 0,
                "pendingUserTurnRequired": true,
                "pendingReason": code,
                "firstPassValid": false,
                "dryRunValid": false,
                "structuredFailureMode": code,
                "proseOnlyFailure": false
            },
            "acpMechanismSummary": ["state_anchor_provider_router", "structured_pending_question", "dry_run_only"]
        },
        "trace": [
            {"phase": "state_anchor_provider", "status": "pending", "message": code}
        ]
    })
}

#[allow(clippy::too_many_arguments)]
fn deal_trace_projection(
    outcome: &str,
    language_pack: &crate::runbook::SemOsLanguagePack,
    dry_run: Option<&crate::runbook::DslDrafterDryRunResult>,
    transition_ref: Option<&str>,
    requested_state: Option<&str>,
    refusal_code: Option<&str>,
    diagnostic_codes: Vec<String>,
    needed_from_user: Vec<String>,
) -> Value {
    let current_state = language_pack.current_state.as_str();
    let requested_state =
        requested_state.or_else(|| dry_run.map(|dry_run| dry_run.semantic_diff.to_state.as_str()));
    let transition_ref =
        transition_ref.or_else(|| dry_run.map(|dry_run| dry_run.transition_ref.as_str()));
    let human_summary = match outcome {
        "dry_run_validated" => format!(
            "I found a valid deal.update-status transition from {} to {} and drafted a dry-run workbook; no mutation was executed.",
            current_state,
            requested_state.unwrap_or("the requested state")
        ),
        "structured_refusal" => format!(
            "I stopped with structured refusal {}; no mutation was executed.",
            refusal_code.unwrap_or("deal_update_status_refused")
        ),
        _ => "ACP state-anchor provider produced a structured outcome.".to_string(),
    };
    let state_anchor_source = if language_pack.state_snapshot_id.starts_with("postgres:") {
        "live_read_only_discovery_probe"
    } else {
        "prompt_read_only_state_anchor"
    };
    json!({
        "outcome": outcome,
        "packId": language_pack.pack_id,
        "packRef": format!("{}@{}", language_pack.pack_id, language_pack.pack_version),
        "subjectId": language_pack.subject.id,
        "verb": DEAL_UPDATE_STATUS_TASK,
        "currentState": current_state,
        "requestedState": requested_state,
        "transitionRef": transition_ref,
        "workbookId": dry_run.map(|dry_run| dry_run.workbook_id.as_str()),
        "semanticDiffUri": dry_run.map(|dry_run| dry_run.semantic_diff_uri.as_str()),
        "promptContextVariant": "deterministic_language_loop",
        "decodeRepairCount": 0,
        "revisionCount": 0,
        "outcomeLayer": outcome,
        "diagnosticCodes": diagnostic_codes,
        "firstPassValid": outcome == "dry_run_validated",
        "dryRunValid": outcome == "dry_run_validated",
        "humanSummary": human_summary,
        "neededFromUser": needed_from_user,
        "stateDiscovery": {
            "source": state_anchor_source,
            "subjectKind": "deal",
            "subjectId": language_pack.subject.id,
            "currentState": current_state,
            "configurationVersion": language_pack.configuration_version,
            "stateSnapshotId": language_pack.state_snapshot_id
        }
    })
}

fn acp_state_anchor_structured_outgoing(
    session_id: Uuid,
    response_id: Value,
    title: &str,
    result: Value,
    message: impl Into<String>,
) -> Vec<crate::acp_protocol::JsonRpcOutgoing> {
    let trace_projection = result
        .get("traceProjection")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let language_pack_uri = result
        .get("language_pack")
        .and_then(|pack| pack.get("pack_id"))
        .and_then(Value::as_str)
        .map(|pack_id| format!("semos://pack-manifest/{pack_id}"));
    let status = result
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("pending_question");
    let subject_id = trace_projection
        .get("subjectId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let validation_status = if status == "dry_run_validated" {
        "completed"
    } else {
        "failed"
    };

    let mut outgoing = Vec::new();
    if let Some(subject_id) = &subject_id {
        outgoing.push(crate::acp_protocol::JsonRpcOutgoing::Notification(
            crate::acp_protocol::JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "session/update".to_string(),
                params: json!({
                    "sessionId": session_id.to_string(),
                    "update": {
                        "sessionUpdate": "tool_call_update",
                        "toolCallId": format!("tool:deal-state-discovery:{subject_id}"),
                        "status": "completed",
                        "kind": "read",
                        "persona": crate::acp::AcpPersonaMode::SagePlanning.as_str(),
                        "workflowPhase": "discovery",
                        "title": "Deal state discovery",
                        "traceProjection": trace_projection,
                        "content": {
                            "type": "resource_link",
                            "uri": format!("semos://entity/{subject_id}"),
                            "name": "Read-only deal state anchor",
                            "description": "Resolved current deal_status, configuration version, and state snapshot before workbook drafting"
                        }
                    }
                }),
            },
        ));
    }
    if let Some(uri) = language_pack_uri {
        outgoing.push(crate::acp_protocol::JsonRpcOutgoing::Notification(
            crate::acp_protocol::JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "session/update".to_string(),
                params: json!({
                    "sessionId": session_id.to_string(),
                    "update": {
                        "sessionUpdate": "tool_call_update",
                        "toolCallId": format!("tool:language-pack:{session_id}"),
                        "status": "completed",
                        "kind": "read",
                        "persona": crate::acp::AcpPersonaMode::SagePlanning.as_str(),
                        "workflowPhase": "discovery",
                        "title": "SemOS language pack",
                        "content": {
                            "type": "resource_link",
                            "uri": uri,
                            "name": "Deal update-status language pack",
                            "description": "Bounded private DSL context for deal.update-status"
                        }
                    }
                }),
            },
        ));
    }
    outgoing.push(crate::acp_protocol::JsonRpcOutgoing::Notification(
        crate::acp_protocol::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "session/update".to_string(),
            params: json!({
                "sessionId": session_id.to_string(),
                "update": {
                    "sessionUpdate": "plan",
                    "persona": crate::acp::AcpPersonaMode::SagePlanning.as_str(),
                    "workflowPhase": "planning",
                    "goalProposalTrace": result,
                    "entries": [
                        {"id": "language-pack", "status": "completed", "label": "Retrieve task language pack"},
                        {"id": "draft", "status": "completed", "label": "Draft workbook"},
                        {"id": "validate", "status": validation_status, "label": "Validate workbook"},
                        {"id": "dry-run", "status": validation_status, "label": "Dry-run only"}
                    ]
                }
            }),
        },
    ));
    outgoing.push(crate::acp_protocol::JsonRpcOutgoing::Notification(
        crate::acp_protocol::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "session/update".to_string(),
            params: json!({
                "sessionId": session_id.to_string(),
                "update": {
                    "sessionUpdate": "tool_call_update",
                    "toolCallId": format!("tool:language-loop:{session_id}"),
                    "status": validation_status,
                    "kind": "think",
                    "persona": crate::acp::AcpPersonaMode::SageExecution.as_str(),
                    "workflowPhase": "planning",
                    "title": title,
                    "traceProjection": result["traceProjection"].clone()
                }
            }),
        },
    ));
    if let Some(dry_run) = result
        .get("output")
        .and_then(|output| output.get("dry_run"))
    {
        outgoing.push(crate::acp_protocol::JsonRpcOutgoing::Notification(
            crate::acp_protocol::JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "session/update".to_string(),
                params: json!({
                    "sessionId": session_id.to_string(),
                    "update": {
                        "sessionUpdate": "semantic_diff",
                        "persona": crate::acp::AcpPersonaMode::SageExecution.as_str(),
                        "semanticDiffId": dry_run["semantic_diff_uri"].clone(),
                        "fallbackSummary": ["resource_link"],
                        "diff": dry_run["semantic_diff"]["semantic_diff"].clone(),
                        "transitionRef": dry_run["transition_ref"].clone(),
                        "validationTrace": dry_run["validation_trace"].clone()
                    }
                }),
            },
        ));
    }
    outgoing.push(crate::acp_protocol::JsonRpcOutgoing::Notification(
        crate::acp_protocol::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "session/update".to_string(),
            params: json!({
                "sessionId": session_id.to_string(),
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": {"type": "text", "text": message.into()}
                }
            }),
        },
    ));
    outgoing.push(crate::acp_protocol::JsonRpcOutgoing::Response(
        crate::acp_protocol::JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(response_id),
            result: Some(result),
            error: None,
        },
    ));
    outgoing
}

fn zero_performance() -> Value {
    json!({
        "prompt_route_ms": 0,
        "prompt_route_us": 0,
        "language_pack_ms": 0,
        "language_pack_us": 0,
        "revision_loop_ms": 0,
        "revision_loop_us": 0,
        "dry_run_ms": 0,
        "dry_run_us": 0,
        "acp_emit_ms": 0,
        "acp_emit_us": 0,
        "total_ms": 0,
        "total_us": 0
    })
}

fn performance(total_us: u64) -> Value {
    json!({
        "prompt_route_ms": millis_from_micros(total_us),
        "prompt_route_us": total_us,
        "language_pack_ms": 0,
        "language_pack_us": 0,
        "revision_loop_ms": 0,
        "revision_loop_us": 0,
        "dry_run_ms": millis_from_micros(total_us),
        "dry_run_us": total_us,
        "acp_emit_ms": 0,
        "acp_emit_us": 0,
        "total_ms": millis_from_micros(total_us),
        "total_us": total_us
    })
}

fn elapsed_us(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn millis_from_micros(micros: u64) -> f64 {
    (micros as f64) / 1_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::uuid;

    const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    const SUBJECT_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");

    fn text_prompt(text: impl Into<String>) -> Vec<crate::acp_protocol::AcpContentBlock> {
        vec![crate::acp_protocol::AcpContentBlock::Text { text: text.into() }]
    }

    fn discovery_resource(
        current_state: &str,
        configuration_version: &str,
        state_snapshot_id: &str,
    ) -> crate::acp_protocol::AcpContentBlock {
        crate::acp_protocol::AcpContentBlock::EmbeddedResource {
            uri: format!("semos://entity/{SUBJECT_ID}"),
            name: Some("KYC case-state discovery".to_string()),
            mime_type: Some("application/json".to_string()),
            text: Some(
                json!({
                    "probe_id": "kyc-case.read-state",
                    "subject": {
                        "subject_kind": "kyc_case",
                        "subject_id": SUBJECT_ID
                    },
                    "case_state": {
                        "subject_id": SUBJECT_ID,
                        "current_state": current_state,
                        "configuration_version": configuration_version,
                        "state_snapshot_id": state_snapshot_id
                    }
                })
                .to_string(),
            ),
        }
    }

    #[test]
    fn provider_registry_declares_dry_run_only_non_mutating_providers() {
        let registry = provider_registry();

        assert!(registry
            .iter()
            .any(|provider| provider.task == KYC_UPDATE_STATUS_TASK));
        assert!(registry
            .iter()
            .any(|provider| provider.task == DEAL_UPDATE_STATUS_TASK));
        assert!(registry
            .iter()
            .any(|provider| provider.task == DEAL_UPDATE_STATUS_TASK
                && provider.language_pack_boundary == "update_status_language_pack_v1"));
        assert!(registry.iter().all(|provider| provider.dry_run_only));
        assert!(registry.iter().all(|provider| !provider.mutation_authority));
        assert!(registry
            .iter()
            .all(|provider| !provider.supported_verbs.is_empty()));
    }

    #[test]
    fn live_case_state_discovery_request_is_read_only_gateway_probe() {
        let request = acp_prompt_live_case_state_discovery_request(
            SESSION_ID,
            AcpPromptLiveCaseState {
                subject_id: SUBJECT_ID,
                current_state: "DISCOVERY".to_string(),
                configuration_version: "domain_pack:ob-poc.kyc@0.1.0".to_string(),
                state_snapshot_id:
                    "postgres:ob-poc.cases:11111111-1111-1111-1111-111111111111:status:discovery"
                        .to_string(),
            },
        );

        assert_eq!(request.method, "obpoc/kyc_case_state/discover");
        assert_eq!(request.params["first_class_state_mutated"], false);
        assert_eq!(request.params["subject_id"], SUBJECT_ID.to_string());
        assert_eq!(request.params["observations"][0]["key"], "case.status");
        assert_eq!(
            request.params["provenance"][0]["source"],
            "postgres.ob-poc.cases"
        );
    }

    #[test]
    fn kyc_case_id_extraction_and_anchor_detection_are_preserved() {
        let prompt = vec![
            crate::acp_protocol::AcpContentBlock::Text {
                text: format!(
                    "Advance KYC case {SUBJECT_ID} to ASSESSMENT with evidence sha256:evidence"
                ),
            },
            discovery_resource("DISCOVERY", "config-1", "snapshot-1"),
        ];

        assert!(acp_prompt_looks_like_kyc_update_status(&prompt));
        assert!(acp_prompt_has_read_only_case_state_anchor(&prompt));
        assert_eq!(acp_prompt_case_id_from_prompt(&prompt), Some(SUBJECT_ID));
    }

    #[test]
    fn provider_selection_is_task_bounded() {
        let kyc_prompt = text_prompt(format!(
            "Advance KYC case {SUBJECT_ID} to DISCOVERY with evidence sha256:evidence"
        ));
        let deal_prompt = text_prompt(format!(
            "Advance deal {SUBJECT_ID} from PROSPECT to QUALIFYING with evidence sha256:evidence"
        ));
        let loan_prompt = text_prompt(format!(
            "Advance loan case {SUBJECT_ID} to APPROVED with evidence sha256:evidence"
        ));

        assert_eq!(
            acp_prompt_state_anchor_provider_selection(&kyc_prompt),
            Some(AcpPromptStateAnchorProviderSelection::Provider(
                AcpPromptStateAnchorProvider::KycUpdateStatus
            ))
        );
        assert_eq!(
            acp_prompt_state_anchor_provider_selection(&deal_prompt),
            Some(AcpPromptStateAnchorProviderSelection::Provider(
                AcpPromptStateAnchorProvider::DealUpdateStatus
            ))
        );
        assert_eq!(
            acp_prompt_state_anchor_provider_selection(&loan_prompt),
            Some(AcpPromptStateAnchorProviderSelection::UnsupportedStatefulDag)
        );
    }

    #[test]
    fn deal_update_status_dry_run_uses_generic_workbook_gate() {
        let language_pack = deal_update_status_language_pack(
            SUBJECT_ID,
            "PROSPECT",
            deal_configuration_version(),
            "prompt:deal:test:deal_status:prospect".to_string(),
            None,
        );
        let transition =
            deal_update_status_transition("PROSPECT", "QUALIFYING").expect("transition exists");
        let value = build_deal_update_status_dry_run_value(
            SESSION_ID,
            &language_pack,
            transition,
            "sha256:evidence".to_string(),
            100,
        )
        .expect("deal dry-run value");

        assert_eq!(value["status"], "dry_run_validated");
        assert_eq!(
            value["output"]["dry_run"]["transition_ref"],
            "deal.prospect-to-qualifying"
        );
        assert_eq!(
            value["output"]["dry_run"]["semantic_diff"]["semantic_diff"]["field"],
            "deal_status"
        );
        assert_eq!(value["metrics"]["dry_run_valid"], true);
    }

    #[test]
    fn deal_prompt_parses_states_and_evidence() {
        let prompt = text_prompt(format!(
            "Advance deal {SUBJECT_ID} from IN_CLEARANCE to CONTRACTED with evidence sha256:abc123"
        ));

        assert_eq!(
            acp_prompt_explicit_current_deal_state(&prompt).as_deref(),
            Some("IN_CLEARANCE")
        );
        assert_eq!(
            acp_prompt_requested_deal_state(&prompt).as_deref(),
            Some("CONTRACTED")
        );
        assert_eq!(
            acp_prompt_evidence_digest(&prompt).as_deref(),
            Some("sha256:abc123")
        );
    }
}
