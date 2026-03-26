//! Phase 5 execution trace helpers.

use serde_json::json;

/// Evaluated Phase 5 result for a single turn.
#[derive(Debug, Clone)]
pub struct Phase5Evaluation {
    pub payload: serde_json::Value,
    pub execution_shape_kind: Option<String>,
}

impl Phase5Evaluation {
    /// Create a new Phase 5 evaluation wrapper.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase5Evaluation;
    ///
    /// let evaluation = Phase5Evaluation::new(serde_json::json!({"status": "available"}), None);
    /// assert_eq!(evaluation.payload()["status"], "available");
    /// ```
    pub fn new(payload: serde_json::Value, execution_shape_kind: Option<String>) -> Self {
        Self {
            payload,
            execution_shape_kind,
        }
    }

    /// Return the persisted Phase 5 payload for this evaluation.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase5Evaluation;
    ///
    /// let evaluation = Phase5Evaluation::new(serde_json::json!({"status": "available"}), None);
    /// assert_eq!(evaluation.payload()["status"], "available");
    /// ```
    pub fn payload(&self) -> serde_json::Value {
        self.payload.clone()
    }

    /// Return the execution-shape kind hoist for this evaluation.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase5Evaluation;
    ///
    /// let evaluation = Phase5Evaluation::new(serde_json::json!({}), Some("singleton".to_string()));
    /// assert_eq!(evaluation.execution_shape_kind(), Some("singleton"));
    /// ```
    pub fn execution_shape_kind(&self) -> Option<&str> {
        self.execution_shape_kind.as_deref()
    }

    /// Return whether the Phase 5 evaluation has an unavailable payload.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::Phase5Evaluation;
    ///
    /// let evaluation = Phase5Evaluation::new(serde_json::json!({"status": "unavailable"}), None);
    /// assert!(evaluation.is_unavailable());
    /// ```
    pub fn is_unavailable(&self) -> bool {
        self.payload
            .get("status")
            .and_then(serde_json::Value::as_str)
            == Some("unavailable")
    }
}

/// Build a Phase 5 payload from REPL execution responses.
///
/// # Examples
/// ```rust
/// use ob_poc::traceability::build_phase5_unavailable_payload;
///
/// let payload = build_phase5_unavailable_payload("repl_v2");
/// assert_eq!(payload["status"], "unavailable");
/// ```
pub fn build_phase5_unavailable_payload(entrypoint: &str) -> serde_json::Value {
    json!({
        "status": "unavailable",
        "entrypoint": entrypoint,
        "dsl_command": serde_json::Value::Null,
        "execution_start": serde_json::Value::Null,
        "execution_end": serde_json::Value::Null,
        "outcome": serde_json::Value::Null,
        "side_effects": [],
        "post_constellation_snapshot": serde_json::Value::Null,
        "repl_session_id": serde_json::Value::Null,
        "runbook_id": serde_json::Value::Null,
        "execution_shape": serde_json::Value::Null,
        "runtime_plan": serde_json::Value::Null,
    })
}

/// Build a Phase 5 execution payload for the direct agent chat path.
///
/// # Examples
/// ```rust,ignore
/// // Built from a real chat execution response at runtime.
/// ```
#[cfg(feature = "database")]
pub fn build_phase5_agent_payload(
    session: &crate::session::UnifiedSession,
    response: &crate::api::agent_service::AgentChatResponse,
) -> serde_json::Value {
    evaluate_phase5_agent(session, response).payload()
}

/// Evaluate Phase 5 execution for the direct agent chat path.
///
/// # Examples
/// ```rust,ignore
/// // Built from a real chat execution response at runtime.
/// ```
#[cfg(feature = "database")]
pub fn evaluate_phase5_agent(
    session: &crate::session::UnifiedSession,
    response: &crate::api::agent_service::AgentChatResponse,
) -> Phase5Evaluation {
    let step_count = session.run_sheet.entries.len();
    let artifact_count = session.pending_execution_artifacts.len();
    let effective_step_count = if step_count == 0 {
        artifact_count
    } else {
        step_count
    };
    let execution_shape = if effective_step_count == 0 {
        None
    } else if effective_step_count == 1 {
        Some("singleton")
    } else {
        Some("batch")
    };

    let runtime_plan = json!({
        "node_count": effective_step_count,
        "nodes": if step_count == 0 {
            session
                .pending_execution_artifacts
                .iter()
                .enumerate()
                .map(|(index, artifact)| json!({
                    "node_id": artifact.get("step_id").cloned().unwrap_or(serde_json::json!(format!("artifact-{index}"))),
                    "dsl_command": response.dsl_source,
                    "display_dsl": response.dsl_source,
                    "status": artifact.get("status").cloned().unwrap_or(serde_json::Value::Null),
                    "depends_on": serde_json::json!([]),
                }))
                .collect::<Vec<_>>()
        } else {
            session
                .run_sheet
                .entries
                .iter()
                .map(|entry| json!({
                    "node_id": entry.id,
                    "dsl_command": entry.dsl_source,
                    "display_dsl": entry.display_dsl,
                    "status": entry.status,
                    "depends_on": entry.dependencies,
                }))
                .collect::<Vec<_>>()
        },
    });
    let runbook_ids = session
        .pending_execution_artifacts
        .iter()
        .filter_map(|artifact| artifact.get("runbook_id").cloned())
        .collect::<Vec<_>>();
    let side_effects = session
        .pending_execution_artifacts
        .iter()
        .filter_map(|artifact| {
            let result = artifact.get("result")?;
            Some(json!({
                "entity_id": serde_json::Value::Null,
                "field_or_state": artifact.get("verb").cloned().unwrap_or(serde_json::Value::Null),
                "before": serde_json::Value::Null,
                "after": result,
            }))
        })
        .collect::<Vec<_>>();

    let payload = match response.session_state {
        crate::session::SessionState::Executed | crate::session::SessionState::Executing => json!({
            "status": "available",
            "dsl_command": response.dsl_source,
            "execution_start": serde_json::Value::Null,
            "execution_end": serde_json::Value::Null,
            "outcome": {
                "kind": if response.parked_entries.is_some() { "parked" } else { "success" },
                "step_count": effective_step_count,
                "parked_count": response.parked_entries.as_ref().map(|entries| entries.len()).unwrap_or(0),
            },
            "side_effects": side_effects,
            "post_constellation_snapshot": serde_json::Value::Null,
            "repl_session_id": serde_json::Value::Null,
            "runbook_id": runbook_ids.first().cloned().unwrap_or(serde_json::Value::Null),
            "runbook_ids": runbook_ids,
            "execution_shape": execution_shape,
            "runtime_plan": runtime_plan,
            "runtime_rechecks": session.pending_execution_rechecks.clone(),
            "execution_artifacts": session.pending_execution_artifacts.clone(),
        }),
        _ => build_phase5_unavailable_payload("agent_service_direct"),
    };

    Phase5Evaluation::new(payload, execution_shape.map(ToString::to_string))
}

/// Build an execution-shape label from the current runbook structure.
///
/// # Examples
/// ```rust
/// use ob_poc::repl::runbook::Runbook;
/// use ob_poc::traceability::build_repl_execution_shape_kind;
/// use uuid::Uuid;
///
/// let runbook = Runbook::new(Uuid::nil());
/// assert_eq!(build_repl_execution_shape_kind(&runbook), Some("singleton"));
/// ```
pub fn build_repl_execution_shape_kind(
    runbook: &crate::repl::runbook::Runbook,
) -> Option<&'static str> {
    if runbook.entries.is_empty() {
        return None;
    }

    if runbook.entries.len() == 1 {
        return Some("singleton");
    }

    if runbook
        .entries
        .iter()
        .any(|entry| !entry.depends_on.is_empty())
    {
        Some("cross_entity_plan")
    } else {
        Some("batch")
    }
}

/// Build a Phase 5 execution payload from the current REPL response/session.
///
/// # Examples
/// ```rust,ignore
/// // Built from a real REPL response at runtime.
/// ```
pub fn build_phase5_repl_payload(
    session: &crate::repl::session_v2::ReplSessionV2,
    response: &crate::repl::response_v2::ReplResponseV2,
) -> serde_json::Value {
    evaluate_phase5_repl(session, response).payload()
}

/// Evaluate Phase 5 execution for the current REPL response/session.
///
/// # Examples
/// ```rust,ignore
/// // Built from a real REPL response at runtime.
/// ```
pub fn evaluate_phase5_repl(
    session: &crate::repl::session_v2::ReplSessionV2,
    response: &crate::repl::response_v2::ReplResponseV2,
) -> Phase5Evaluation {
    let execution_shape = build_repl_execution_shape_kind(&session.runbook);
    let runtime_plan = repl_runtime_plan(&session.runbook);

    let payload = match &response.kind {
        crate::repl::response_v2::ReplResponseKindV2::Executed { results } => json!({
            "status": "available",
            "dsl_command": serde_json::Value::Null,
            "execution_start": serde_json::Value::Null,
            "execution_end": serde_json::Value::Null,
            "outcome": {
                "kind": "success",
                "step_count": results.len(),
                "successful_steps": results.iter().filter(|step| step.success).count(),
            },
            "side_effects": repl_side_effects(results),
            "post_constellation_snapshot": serde_json::Value::Null,
            "repl_session_id": session.id,
            "runbook_id": session.runbook.id,
            "execution_shape": execution_shape,
            "runtime_plan": runtime_plan,
            "runtime_rechecks": session.pending_execution_rechecks.clone(),
        }),
        crate::repl::response_v2::ReplResponseKindV2::Parked {
            results_so_far,
            parked_entries,
            ..
        } => json!({
            "status": "available",
            "dsl_command": serde_json::Value::Null,
            "execution_start": serde_json::Value::Null,
            "execution_end": serde_json::Value::Null,
            "outcome": {
                "kind": "parked",
                "step_count": results_so_far.len(),
                "parked_count": parked_entries.len(),
            },
            "side_effects": repl_side_effects(results_so_far),
            "post_constellation_snapshot": serde_json::Value::Null,
            "repl_session_id": session.id,
            "runbook_id": session.runbook.id,
            "execution_shape": execution_shape,
            "runtime_plan": runtime_plan,
            "runtime_rechecks": session.pending_execution_rechecks.clone(),
        }),
        _ => build_phase5_unavailable_payload("repl_v2"),
    };

    Phase5Evaluation::new(payload, execution_shape.map(ToString::to_string))
}

fn repl_runtime_plan(runbook: &crate::repl::runbook::Runbook) -> serde_json::Value {
    let nodes = runbook
        .entries
        .iter()
        .map(|entry| {
            json!({
                "node_id": entry.id,
                "sequence": entry.sequence,
                "verb_id": entry.verb,
                "dsl_command": entry.dsl,
                "depends_on": entry.depends_on,
                "execution_mode": entry.execution_mode,
                "status": entry.status,
                "compiled_runbook_id": entry.compiled_runbook_id.map(|id| id.to_string()),
            })
        })
        .collect::<Vec<_>>();

    let dependency_edges = runbook
        .entries
        .iter()
        .flat_map(|entry| {
            entry.depends_on.iter().map(move |dep| {
                json!({
                    "from": dep,
                    "to": entry.id,
                })
            })
        })
        .collect::<Vec<_>>();

    json!({
        "template_id": runbook.template_id,
        "template_hash": runbook.template_hash,
        "pack_id": runbook.pack_id,
        "pack_version": runbook.pack_version,
        "node_count": runbook.entries.len(),
        "edge_count": dependency_edges.len(),
        "nodes": nodes,
        "dependency_edges": dependency_edges,
    })
}

fn repl_side_effects(results: &[crate::repl::response_v2::StepResult]) -> Vec<serde_json::Value> {
    results
        .iter()
        .filter_map(|step| {
            step.result.as_ref().map(|result| {
                json!({
                    "entity_id": serde_json::Value::Null,
                    "field_or_state": step.sentence,
                    "before": serde_json::Value::Null,
                    "after": result,
                })
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "database")]
    use super::{build_phase5_agent_payload, evaluate_phase5_agent};
    use super::{build_phase5_repl_payload, build_repl_execution_shape_kind, evaluate_phase5_repl};
    #[cfg(feature = "database")]
    use crate::api::agent_service::AgentChatResponse;
    use crate::repl::response_v2::{ReplResponseKindV2, ReplResponseV2, StepResult};
    use crate::repl::runbook::RunbookEntry;
    use crate::repl::session_v2::ReplSessionV2;
    use crate::repl::types_v2::{ExecutionProgress, ReplStateV2};
    #[cfg(feature = "database")]
    use crate::session::{SessionState, UnifiedSession};
    #[cfg(feature = "database")]
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_phase5_repl_payload_for_executed_response() {
        let response = ReplResponseV2 {
            state: ReplStateV2::Executing {
                runbook_id: Uuid::new_v4(),
                progress: ExecutionProgress::new(1),
            },
            kind: ReplResponseKindV2::Executed {
                results: vec![StepResult {
                    entry_id: Uuid::new_v4(),
                    sequence: 1,
                    sentence: "Open case".to_string(),
                    success: true,
                    message: Some("ok".to_string()),
                    result: Some(serde_json::json!({"case_id": "abc"})),
                }],
            },
            message: "done".to_string(),
            runbook_summary: None,
            step_count: 1,
            session_feedback: None,
        };

        let session = ReplSessionV2::new();
        let payload = build_phase5_repl_payload(&session, &response);
        assert_eq!(payload["outcome"]["kind"], "success");
    }

    #[test]
    fn test_execution_shape_kind_prefers_cross_entity_plan_when_dependencies_exist() {
        let mut runbook = crate::repl::runbook::Runbook::new(Uuid::new_v4());
        let first = RunbookEntry::new(
            "cbu.create".to_string(),
            "Create CBU".to_string(),
            "(cbu.create)".to_string(),
        );
        let first_id = first.id;
        let mut second = RunbookEntry::new(
            "case.open".to_string(),
            "Open case".to_string(),
            "(case.open)".to_string(),
        );
        second.depends_on.push(first_id);
        runbook.add_entry(first);
        runbook.add_entry(second);

        assert_eq!(
            build_repl_execution_shape_kind(&runbook),
            Some("cross_entity_plan")
        );
    }

    #[test]
    fn test_phase5_payload_includes_runtime_plan() {
        let mut session = ReplSessionV2::new();
        session.runbook.template_id = Some("basic-onboarding".to_string());
        let first = RunbookEntry::new(
            "cbu.create".to_string(),
            "Create CBU".to_string(),
            "(cbu.create)".to_string(),
        );
        let first_id = first.id;
        let mut second = RunbookEntry::new(
            "case.open".to_string(),
            "Open case".to_string(),
            "(case.open)".to_string(),
        );
        second.depends_on.push(first_id);
        session.runbook.add_entry(first);
        session.runbook.add_entry(second);

        let response = ReplResponseV2 {
            state: ReplStateV2::Executing {
                runbook_id: session.runbook.id,
                progress: ExecutionProgress::new(2),
            },
            kind: ReplResponseKindV2::Executed { results: vec![] },
            message: "done".to_string(),
            runbook_summary: None,
            step_count: 2,
            session_feedback: None,
        };

        let payload = build_phase5_repl_payload(&session, &response);
        assert_eq!(payload["execution_shape"], "cross_entity_plan");
        assert_eq!(payload["runtime_plan"]["template_id"], "basic-onboarding");
        assert_eq!(payload["runtime_plan"]["node_count"], 2);
        assert_eq!(payload["runtime_plan"]["edge_count"], 1);
    }

    #[test]
    fn test_phase5_repl_evaluation_exposes_execution_shape() {
        let mut session = ReplSessionV2::new();
        session.runbook.add_entry(RunbookEntry::new(
            "case.open".to_string(),
            "Open case".to_string(),
            "(case.open)".to_string(),
        ));

        let response = ReplResponseV2 {
            state: ReplStateV2::Executing {
                runbook_id: session.runbook.id,
                progress: ExecutionProgress::new(1),
            },
            kind: ReplResponseKindV2::Executed { results: vec![] },
            message: "done".to_string(),
            runbook_summary: None,
            step_count: 1,
            session_feedback: None,
        };

        let evaluation = evaluate_phase5_repl(&session, &response);
        assert_eq!(evaluation.execution_shape_kind(), Some("singleton"));
        assert!(!evaluation.is_unavailable());
        assert_eq!(evaluation.payload()["status"], "available");
    }

    #[cfg(feature = "database")]
    #[test]
    fn test_phase5_agent_payload_includes_runtime_rechecks() {
        let mut session = UnifiedSession::new();
        session.pending_execution_rechecks = vec![json!({
            "verb": "cbu.create",
            "status": "allowed"
        })];
        session.pending_execution_artifacts = vec![json!({
            "runbook_id": "rb-1",
            "step_id": Uuid::nil(),
            "verb": "cbu.create",
            "status": "completed",
            "result": {"cbu_id": "abc"}
        })];

        let response = AgentChatResponse {
            message: "done".to_string(),
            session_state: SessionState::Executed,
            can_execute: false,
            dsl_source: Some("(cbu.create)".to_string()),
            ast: None,
            disambiguation: None,
            commands: None,
            unresolved_refs: None,
            current_ref_index: None,
            dsl_hash: None,
            verb_disambiguation: None,
            intent_tier: None,
            decision: None,
            sage_explain: None,
            coder_proposal: None,
            discovery_bootstrap: None,
            parked_entries: None,
            onboarding_state: None,
        };

        let payload = build_phase5_agent_payload(&session, &response);
        assert_eq!(payload["status"], "available");
        assert_eq!(payload["runtime_rechecks"][0]["status"], "allowed");
        assert_eq!(payload["runbook_id"], "rb-1");
        assert_eq!(payload["execution_artifacts"][0]["status"], "completed");
    }

    #[cfg(feature = "database")]
    #[test]
    fn test_phase5_agent_evaluation_exposes_execution_shape() {
        let mut session = UnifiedSession::new();
        session.pending_execution_artifacts.push(serde_json::json!({
            "runbook_id": "rb-1",
            "step_id": "step-1",
            "verb": "case.open",
            "status": "completed",
            "final_status": "executed",
            "result": null,
        }));

        let response = AgentChatResponse {
            message: "done".to_string(),
            session_state: SessionState::Executed,
            can_execute: false,
            dsl_source: Some("(case.open)".to_string()),
            ast: None,
            disambiguation: None,
            commands: None,
            unresolved_refs: None,
            current_ref_index: None,
            dsl_hash: None,
            verb_disambiguation: None,
            intent_tier: None,
            decision: None,
            sage_explain: None,
            coder_proposal: None,
            discovery_bootstrap: None,
            parked_entries: None,
            onboarding_state: None,
        };

        let evaluation = evaluate_phase5_agent(&session, &response);
        assert_eq!(evaluation.execution_shape_kind(), Some("singleton"));
        assert!(!evaluation.is_unavailable());
        assert_eq!(evaluation.payload()["status"], "available");
    }
}
