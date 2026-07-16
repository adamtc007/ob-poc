//! Repl-coupled session-source builders for the ACP runtime context.
//!
//! Phase 3 slice 2d.4 (2026-05-12): the pure boundary surface (the
//! `AcpRuntimeContextSource` / `AcpRuntimeContextProjection` types and the
//! `build_acp_runtime_context_projection` redaction/hashing function) is
//! relocated to `ob_poc_boundary::acp_runtime_context`. This file retains the
//! Repl-coupled internals (`AcpRuntimeContextBuildInput`,
//! `build_session_runtime_context_source`, helpers) that read a live
//! `ReplSessionV2` to populate a runtime source. Those helpers reference
//! execution-tier session types (`ReplSessionV2`, `ReplStateV2`) and stay in
//! ob-poc until the broader repl::session_v2 inversion lands.

// Re-export the pure boundary surface so existing
// `crate::acp_runtime_context::{AcpRuntimeContextSource,
// AcpRuntimeContextProjection, AcpRuntimeContextDiagnostic,
// build_acp_runtime_context_projection, acp_runtime_context_field_allowed,
// ACP_RUNTIME_CONTEXT_*}` callers continue working unchanged.
pub use ob_poc_boundary::acp_runtime_context::*;

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::repl::session_v2::ReplSessionV2;
use crate::repl::types_v2::ReplStateV2;
use crate::runbook::plan_types::{PlanStepStatus, RunbookPlanStatus};

// ---------------------------------------------------------------------------
// Session-derived source collection (formerly `acp_runtime_context_sources`).
// Transport-adjacent: reads already scoped session state and produces an
// `AcpRuntimeContextSource`. Redaction and deterministic hashing remain above.
// ---------------------------------------------------------------------------

/// Input required to build a request-scoped runtime source from a REPL session.
#[derive(Debug, Clone)]
pub(crate) struct AcpRuntimeContextBuildInput<'a> {
    pub(crate) pack_id: String,
    pub(crate) selected_ref: String,
    pub(crate) static_envelope_hash: String,
    pub(crate) session: Option<&'a ReplSessionV2>,
    pub(crate) missing_required_args: Vec<String>,
}

/// Build a scoped runtime source from a single `ReplSessionV2` snapshot.
pub(crate) fn build_session_runtime_context_source(
    input: AcpRuntimeContextBuildInput<'_>,
) -> AcpRuntimeContextSource {
    let Some(session) = input.session else {
        return missing_session_source(input);
    };

    let mut fields = BTreeMap::new();
    fields.insert(
        "binding_status".to_string(),
        serde_json::json!("session_scoped"),
    );
    fields.insert(
        "fsm_state".to_string(),
        serde_json::json!(repl_state_code(&session.state)),
    );
    if !input.missing_required_args.is_empty() {
        fields.insert(
            "missing_binding_codes".to_string(),
            serde_json::json!(input.missing_required_args),
        );
    }

    match input.pack_id.as_str() {
        "onboarding-request" => add_onboarding_fields(session, &mut fields),
        "cbu-maintenance" => add_cbu_fields(session, &mut fields),
        "product-service-taxonomy" => add_taxonomy_fields(session, &mut fields),
        _ => {}
    }

    let snapshot_id = session_snapshot_id(
        session,
        &input.pack_id,
        &input.selected_ref,
        &input.static_envelope_hash,
    );
    let mut source_refs = vec![
        format!("repl_session:{}", session.id),
        format!(
            "repl_session:last_active_at:{}",
            session.last_active_at.to_rfc3339()
        ),
    ];
    if let Some(plan) = &session.runbook_plan {
        source_refs.push(format!("runbook_plan:{}", plan.id));
    }
    if !session.runbook.entries.is_empty() {
        source_refs.push(format!("runbook:{}", session.runbook.id));
    }

    AcpRuntimeContextSource {
        pack_id: input.pack_id,
        session_id: Some(session.id.to_string()),
        snapshot_id,
        snapshot_created_at: session.last_active_at.to_rfc3339(),
        source_refs,
        static_envelope_hash: input.static_envelope_hash,
        fields,
        stale: false,
        missing_source_codes: Vec::new(),
        force_count_only: false,
        field_budget: Some(12),
    }
}

fn missing_session_source(input: AcpRuntimeContextBuildInput<'_>) -> AcpRuntimeContextSource {
    let snapshot_id = format!(
        "repl-session-missing:{}",
        stable_hash(&format!(
            "{}:{}:{}",
            input.pack_id, input.selected_ref, input.static_envelope_hash
        ))
    );
    AcpRuntimeContextSource {
        pack_id: input.pack_id,
        session_id: None,
        snapshot_id,
        snapshot_created_at: "request_scoped".to_string(),
        source_refs: vec!["repl_session:missing".to_string()],
        static_envelope_hash: input.static_envelope_hash,
        fields: BTreeMap::new(),
        stale: false,
        missing_source_codes: vec!["runtime_source_unavailable".to_string()],
        force_count_only: false,
        field_budget: Some(12),
    }
}

fn add_onboarding_fields(
    session: &ReplSessionV2,
    fields: &mut BTreeMap<String, serde_json::Value>,
) {
    if let Some(plan) = &session.runbook_plan {
        fields.insert(
            "workbook_step_statuses".to_string(),
            serde_json::json!(plan
                .steps
                .iter()
                .map(|step| serde_json::json!({
                    "step_id": format!("{}:{}", step.seq, step.verb.verb_fqn),
                    "status": plan_step_status_code(step.status),
                }))
                .collect::<Vec<_>>()),
        );
        fields.insert(
            "fsm_state".to_string(),
            serde_json::json!(runbook_plan_status_code(&plan.status)),
        );
    }
    if let Some(cursor) = session.runbook_plan_cursor {
        fields.insert("run_sheet_cursor".to_string(), serde_json::json!(cursor));
    }
    if !session.cbu_ids.is_empty() {
        fields.insert(
            "expected_slice_count".to_string(),
            serde_json::json!(session.cbu_ids.len()),
        );
    }
}

fn add_cbu_fields(session: &ReplSessionV2, fields: &mut BTreeMap<String, serde_json::Value>) {
    if let Some(cbu_id) = session.cbu_ids.last() {
        fields.insert("cbu_id".to_string(), serde_json::json!(cbu_id.to_string()));
    }
    let product_binding_ids = binding_uuid_values(session, "product", 10);
    if !product_binding_ids.is_empty() {
        fields.insert(
            "product_binding_ids".to_string(),
            serde_json::json!(product_binding_ids),
        );
    }
}

fn add_taxonomy_fields(session: &ReplSessionV2, fields: &mut BTreeMap<String, serde_json::Value>) {
    let discovered_srdef_ids = binding_uuid_values(session, "srdef", 10);
    if !discovered_srdef_ids.is_empty() {
        fields.insert(
            "active_srdef_count".to_string(),
            serde_json::json!(discovered_srdef_ids.len()),
        );
        fields.insert(
            "discovered_srdef_ids".to_string(),
            serde_json::json!(discovered_srdef_ids),
        );
    }
    let missing_resource_codes = binding_code_values(session, "missing_resource", 10);
    if !missing_resource_codes.is_empty() {
        fields.insert(
            "missing_resource_count".to_string(),
            serde_json::json!(missing_resource_codes.len()),
        );
        fields.insert(
            "missing_resource_codes".to_string(),
            serde_json::json!(missing_resource_codes),
        );
        fields.insert("operation_required_count".to_string(), serde_json::json!(1));
    }
}

fn binding_uuid_values(session: &ReplSessionV2, key_needle: &str, limit: usize) -> Vec<String> {
    let mut values = Vec::new();
    for (key, value) in &session.bindings {
        if !key.to_ascii_lowercase().contains(key_needle) {
            continue;
        }
        collect_uuid_strings(value, &mut values, limit);
        if values.len() >= limit {
            break;
        }
    }
    values.sort();
    values.dedup();
    values.truncate(limit);
    values
}

fn collect_uuid_strings(value: &serde_json::Value, values: &mut Vec<String>, limit: usize) {
    if values.len() >= limit {
        return;
    }
    match value {
        serde_json::Value::String(candidate) => {
            if Uuid::parse_str(candidate).is_ok() {
                values.push(candidate.clone());
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_uuid_strings(item, values, limit);
                if values.len() >= limit {
                    break;
                }
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values() {
                collect_uuid_strings(item, values, limit);
                if values.len() >= limit {
                    break;
                }
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn binding_code_values(session: &ReplSessionV2, key_needle: &str, limit: usize) -> Vec<String> {
    let mut values = Vec::new();
    for (key, value) in &session.bindings {
        if !key.to_ascii_lowercase().contains(key_needle) {
            continue;
        }
        collect_code_strings(value, &mut values, limit);
        if values.len() >= limit {
            break;
        }
    }
    values.sort();
    values.dedup();
    values.truncate(limit);
    values
}

fn collect_code_strings(value: &serde_json::Value, values: &mut Vec<String>, limit: usize) {
    if values.len() >= limit {
        return;
    }
    match value {
        serde_json::Value::String(candidate) if is_runtime_code(candidate) => {
            values.push(candidate.clone());
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_code_strings(item, values, limit);
                if values.len() >= limit {
                    break;
                }
            }
        }
        serde_json::Value::Object(object) => {
            for (key, item) in object {
                if key.contains("name") || key.contains("label") || key.contains("text") {
                    continue;
                }
                collect_code_strings(item, values, limit);
                if values.len() >= limit {
                    break;
                }
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}

fn is_runtime_code(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 64
        && !trimmed.chars().any(char::is_whitespace)
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.'))
}

fn repl_state_code(state: &ReplStateV2) -> &'static str {
    match state {
        ReplStateV2::ScopeGate { .. } => "scope_gate",
        ReplStateV2::WorkspaceSelection { .. } => "workspace_selection",
        ReplStateV2::ConstellationMapSelection { .. } => "constellation_map_selection",
        ReplStateV2::JourneySelection { .. } => "journey_selection",
        ReplStateV2::InPack { .. } => "in_pack",
        ReplStateV2::Clarifying { .. } => "clarifying",
        ReplStateV2::PackMismatchConfirm { .. } => "pack_mismatch_confirm",
        ReplStateV2::SentencePlayback { .. } => "sentence_playback",
        ReplStateV2::RunbookEditing => "runbook_editing",
        ReplStateV2::Executing { .. } => "executing",
    }
}

fn plan_step_status_code(status: PlanStepStatus) -> &'static str {
    match status {
        PlanStepStatus::Pending => "pending",
        PlanStepStatus::Ready => "ready",
        PlanStepStatus::Executing => "executing",
        PlanStepStatus::Succeeded => "succeeded",
        PlanStepStatus::Failed => "failed",
        PlanStepStatus::Skipped => "skipped",
    }
}

fn runbook_plan_status_code(status: &RunbookPlanStatus) -> &'static str {
    match status {
        RunbookPlanStatus::Compiled => "compiled",
        RunbookPlanStatus::AwaitingApproval => "awaiting_approval",
        RunbookPlanStatus::Approved => "approved",
        RunbookPlanStatus::Executing { .. } => "executing",
        RunbookPlanStatus::Completed { .. } => "completed",
        RunbookPlanStatus::Failed { .. } => "failed",
        RunbookPlanStatus::Cancelled => "cancelled",
    }
}

fn session_snapshot_id(
    session: &ReplSessionV2,
    pack_id: &str,
    selected_ref: &str,
    static_envelope_hash: &str,
) -> String {
    format!(
        "repl-session:{}",
        stable_hash(&format!(
            "{}:{}:{}:{}",
            session.id,
            session.last_active_at.to_rfc3339(),
            pack_id,
            stable_hash(&format!("{selected_ref}:{static_envelope_hash}"))
        ))
    )
}

fn stable_hash(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_denied_fields_and_hashes_deterministically() {
        let mut fields = BTreeMap::new();
        fields.insert("request_state".to_string(), serde_json::json!("ready"));
        fields.insert(
            "owner_email".to_string(),
            serde_json::json!("owner@example.com"),
        );
        let source = AcpRuntimeContextSource {
            pack_id: "onboarding-request".to_string(),
            session_id: Some("session-1".to_string()),
            snapshot_id: "snapshot-1".to_string(),
            snapshot_created_at: "2026-05-10T20:00:00Z".to_string(),
            source_refs: vec!["session:session-1".to_string()],
            static_envelope_hash: "static-hash".to_string(),
            fields,
            stale: false,
            missing_source_codes: Vec::new(),
            force_count_only: false,
            field_budget: None,
        };

        let first = build_acp_runtime_context_projection(source.clone());
        let second = build_acp_runtime_context_projection(source);

        assert_eq!(first.runtime_hash, second.runtime_hash);
        assert!(first.verified);
        assert!(first.runtime_fields.contains_key("request_state"));
        assert!(!first.runtime_fields.contains_key("owner_email"));
        assert_eq!(first.blocked_field_codes, vec!["field.owner_email"]);
    }

    // Tests moved from former `acp_runtime_context_sources` module.

    use crate::repl::types_v2::{SubjectKind, VerbRef, WorkspaceKind};
    use crate::runbook::plan_types::{BindingTable, EntityBinding, RunbookPlan, RunbookPlanStep};

    #[test]
    fn builds_onboarding_source_from_runbook_plan_without_labels() {
        let mut session = ReplSessionV2::new();
        let cbu_id = Uuid::new_v4();
        session.cbu_ids.push(cbu_id);
        session.runbook_plan_cursor = Some(1);
        session.runbook_plan = Some(RunbookPlan::new(
            session.id,
            vec![RunbookPlanStep {
                seq: 1,
                workspace: WorkspaceKind::OnBoarding,
                constellation_map: "onboarding".to_string(),
                subject_kind: SubjectKind::Handoff,
                subject_binding: EntityBinding::Literal { id: cbu_id },
                verb: VerbRef {
                    verb_fqn: "onboarding.compile-data-request".to_string(),
                    display_name: "Compile Data Request".to_string(),
                },
                sentence: "Compile confidential onboarding request".to_string(),
                args: BTreeMap::new(),
                preconditions: Vec::new(),
                expected_effect: "compile request".to_string(),
                depends_on: Vec::new(),
                status: PlanStepStatus::Ready,
            }],
            BindingTable::default(),
            Vec::new(),
        ));

        let source = build_session_runtime_context_source(AcpRuntimeContextBuildInput {
            pack_id: "onboarding-request".to_string(),
            selected_ref: "onboarding.compile-data-request".to_string(),
            static_envelope_hash: "static-hash".to_string(),
            session: Some(&session),
            missing_required_args: vec!["onboarding-request-id".to_string()],
        });
        let projection = build_acp_runtime_context_projection(source);
        let serialized = serde_json::to_string(&projection).expect("projection serializes");

        assert!(projection.verified);
        assert_eq!(projection.pack_id, "onboarding-request");
        assert!(projection
            .runtime_fields
            .contains_key("workbook_step_statuses"));
        assert_eq!(
            projection.runtime_fields["run_sheet_cursor"],
            serde_json::json!(1)
        );
        assert_eq!(
            projection.runtime_fields["missing_binding_codes"],
            serde_json::json!(["onboarding-request-id"])
        );
        assert!(!serialized.contains("Compile confidential onboarding request"));
    }

    #[test]
    fn builds_cbu_source_with_uuid_bindings_only() {
        let mut session = ReplSessionV2::new();
        let cbu_id = Uuid::new_v4();
        let product_id = Uuid::new_v4();
        session.cbu_ids.push(cbu_id);
        session.bindings.insert(
            "product_primary".to_string(),
            serde_json::json!({
                "id": product_id.to_string(),
                "label": "Confidential Product Name"
            }),
        );

        let source = build_session_runtime_context_source(AcpRuntimeContextBuildInput {
            pack_id: "cbu-maintenance".to_string(),
            selected_ref: "cbu.add-product".to_string(),
            static_envelope_hash: "static-hash".to_string(),
            session: Some(&session),
            missing_required_args: Vec::new(),
        });
        let projection = build_acp_runtime_context_projection(source);
        let serialized = serde_json::to_string(&projection).expect("projection serializes");

        assert!(projection.verified);
        assert_eq!(
            projection.runtime_fields["cbu_id"],
            serde_json::json!(cbu_id.to_string())
        );
        assert_eq!(
            projection.runtime_fields["product_binding_ids"],
            serde_json::json!([product_id.to_string()])
        );
        assert!(!serialized.contains("Confidential Product Name"));
    }

    #[test]
    fn missing_session_source_fails_closed() {
        let source = build_session_runtime_context_source(AcpRuntimeContextBuildInput {
            pack_id: "product-service-taxonomy".to_string(),
            selected_ref: "service-resource.list-attributes".to_string(),
            static_envelope_hash: "static-hash".to_string(),
            session: None,
            missing_required_args: Vec::new(),
        });
        let projection = build_acp_runtime_context_projection(source);

        assert!(!projection.verified);
        assert!(projection
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime_context_missing_source"));
        assert_eq!(
            projection.runtime_fields["missing_source_codes"],
            serde_json::json!(["runtime_source_unavailable"])
        );
    }
}
