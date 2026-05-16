//! Bounded Slice 2 runtime context projection for ACP pack routing.
//!
//! This module is deliberately transport-neutral. It turns an already scoped
//! runtime read set into a redacted, deterministic projection that can later be
//! attached to ACP and HTTP traces.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Schema version for Slice 2 ACP runtime context projections.
pub const ACP_RUNTIME_CONTEXT_SCHEMA_VERSION: &str = "acp_runtime_context_v1";

/// Initial deny-by-default redaction policy id for Slice 2 runtime context.
pub const ACP_RUNTIME_CONTEXT_REDACTION_POLICY_V1: &str = "slice2_runtime_context_redaction_v1";

/// Initial request-scoped freshness policy id for Slice 2 runtime context.
pub const ACP_RUNTIME_CONTEXT_FRESHNESS_POLICY_V1: &str = "slice2_runtime_context_same_request_v1";

const DEFAULT_RUNTIME_FIELD_BUDGET: usize = 12;

const ALLOWED_RUNTIME_FIELDS: &[&str] = &[
    "active_srdef_count",
    "attribute_requirement_count",
    "binding_status",
    "blocker_codes",
    "budget_breach_codes",
    "cbu_id",
    "compiled_data_request_id",
    "compiled_data_request_status",
    "count_only_projection",
    "current_phase",
    "discovered_srdef_ids",
    "discovery_freshness_timestamp",
    "expected_slice_count",
    "fsm_state",
    "l4_binding_status",
    "missing_binding_codes",
    "missing_resource_codes",
    "missing_resource_count",
    "missing_source_codes",
    "operation_required_count",
    "owner_coverage_status",
    "product_binding_ids",
    "ready_slice_count",
    "redacted_count",
    "request_id",
    "request_state",
    "resource_fanout_count",
    "run_sheet_cursor",
    "snapshot_id",
    "source_version_refs",
    "workbook_step_statuses",
];

/// Runtime source read set after route/session scoping and before redaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AcpRuntimeContextSource {
    pub pack_id: String,
    pub session_id: Option<String>,
    pub snapshot_id: String,
    pub snapshot_created_at: String,
    pub source_refs: Vec<String>,
    pub static_envelope_hash: String,
    pub fields: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub missing_source_codes: Vec<String>,
    #[serde(default)]
    pub force_count_only: bool,
    #[serde(default)]
    pub field_budget: Option<usize>,
}

/// Redacted, deterministic runtime context projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AcpRuntimeContextProjection {
    pub schema_version: String,
    pub pack_id: String,
    pub session_id: Option<String>,
    pub snapshot_id: String,
    pub snapshot_created_at: String,
    pub source_refs: Vec<String>,
    pub redaction_policy: String,
    pub freshness_policy: String,
    pub runtime_hash: String,
    pub static_envelope_hash: String,
    pub projection_hash: String,
    pub verified: bool,
    pub redacted_count: usize,
    pub blocked_field_codes: Vec<String>,
    pub runtime_fields: BTreeMap<String, serde_json::Value>,
    pub diagnostics: Vec<AcpRuntimeContextDiagnostic>,
}

/// Structured runtime-context diagnostic used for stale, missing, and budget cases.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcpRuntimeContextDiagnostic {
    pub code: String,
    pub source: String,
    pub message: String,
}

/// Build a redacted Slice 2 runtime context projection from a scoped source.
///
/// # Examples
///
/// ```rust
/// use ob_poc_boundary::acp_runtime_context::{
///     build_acp_runtime_context_projection, AcpRuntimeContextSource,
/// };
/// use std::collections::BTreeMap;
///
/// let source = AcpRuntimeContextSource {
///     pack_id: "onboarding-request".to_string(),
///     session_id: Some("session-1".to_string()),
///     snapshot_id: "snapshot-1".to_string(),
///     snapshot_created_at: "2026-05-10T20:00:00Z".to_string(),
///     source_refs: vec!["session:session-1".to_string()],
///     static_envelope_hash: "static-hash".to_string(),
///     fields: BTreeMap::from([(
///         "request_state".to_string(),
///         serde_json::json!("ready"),
///     )]),
///     stale: false,
///     missing_source_codes: Vec::new(),
///     force_count_only: false,
///     field_budget: None,
/// };
///
/// let projection = build_acp_runtime_context_projection(source);
/// assert!(projection.verified);
/// assert!(projection.runtime_fields.contains_key("request_state"));
/// ```
pub fn build_acp_runtime_context_projection(
    source: AcpRuntimeContextSource,
) -> AcpRuntimeContextProjection {
    let budget = source.field_budget.unwrap_or(DEFAULT_RUNTIME_FIELD_BUDGET);
    let allowed_fields = allowed_runtime_fields();
    let mut runtime_fields = BTreeMap::new();
    let mut blocked_field_codes = Vec::new();

    for (field, value) in &source.fields {
        if allowed_fields.contains(field.as_str()) {
            runtime_fields.insert(field.clone(), value.clone());
        } else {
            blocked_field_codes.push(format!("field.{field}"));
        }
    }

    let mut diagnostics = Vec::new();
    if source.stale {
        diagnostics.push(AcpRuntimeContextDiagnostic {
            code: "runtime_context_stale_source_changed".to_string(),
            source: "acp_runtime_context".to_string(),
            message: "Runtime source changed before the projection could be verified".to_string(),
        });
    }
    if !source.missing_source_codes.is_empty() {
        diagnostics.push(AcpRuntimeContextDiagnostic {
            code: "runtime_context_missing_source".to_string(),
            source: "acp_runtime_context".to_string(),
            message: "Required runtime source is unavailable".to_string(),
        });
        runtime_fields.insert(
            "missing_source_codes".to_string(),
            serde_json::json!(source.missing_source_codes),
        );
    }
    if source.force_count_only || runtime_fields.len() > budget {
        diagnostics.push(AcpRuntimeContextDiagnostic {
            code: "runtime_context_budget_count_only".to_string(),
            source: "acp_runtime_context".to_string(),
            message: "Runtime source exceeded projection budget and was reduced to counts"
                .to_string(),
        });
        runtime_fields = count_only_projection(&runtime_fields);
    }

    let redacted_count = blocked_field_codes.len();
    runtime_fields.insert(
        "redacted_count".to_string(),
        serde_json::json!(redacted_count),
    );
    if !blocked_field_codes.is_empty() {
        runtime_fields.insert(
            "blocked_field_codes".to_string(),
            serde_json::json!(blocked_field_codes.clone()),
        );
    }

    let mut projection = AcpRuntimeContextProjection {
        schema_version: ACP_RUNTIME_CONTEXT_SCHEMA_VERSION.to_string(),
        pack_id: source.pack_id,
        session_id: source.session_id,
        snapshot_id: source.snapshot_id,
        snapshot_created_at: source.snapshot_created_at,
        source_refs: source.source_refs,
        redaction_policy: ACP_RUNTIME_CONTEXT_REDACTION_POLICY_V1.to_string(),
        freshness_policy: ACP_RUNTIME_CONTEXT_FRESHNESS_POLICY_V1.to_string(),
        runtime_hash: String::new(),
        static_envelope_hash: source.static_envelope_hash,
        projection_hash: String::new(),
        verified: false,
        redacted_count,
        blocked_field_codes,
        runtime_fields,
        diagnostics,
    };
    projection.runtime_hash = runtime_hash(&projection);
    projection.projection_hash = projection_hash(&projection);
    projection.verified = !projection.static_envelope_hash.is_empty()
        && !projection.snapshot_id.is_empty()
        && !projection.runtime_hash.is_empty()
        && !projection.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "runtime_context_stale_source_changed"
                || diagnostic.code == "runtime_context_missing_source"
        });
    projection
}

/// Return true when a runtime field is allowed by the Slice 2 redaction policy.
///
/// # Examples
///
/// ```rust
/// use ob_poc_boundary::acp_runtime_context::acp_runtime_context_field_allowed;
///
/// assert!(acp_runtime_context_field_allowed("request_state"));
/// assert!(!acp_runtime_context_field_allowed("owner_email"));
/// ```
pub fn acp_runtime_context_field_allowed(field: &str) -> bool {
    allowed_runtime_fields().contains(field)
}

fn allowed_runtime_fields() -> BTreeSet<&'static str> {
    ALLOWED_RUNTIME_FIELDS.iter().copied().collect()
}

fn count_only_projection(
    runtime_fields: &BTreeMap<String, serde_json::Value>,
) -> BTreeMap<String, serde_json::Value> {
    let mut reduced = BTreeMap::new();
    reduced.insert("count_only_projection".to_string(), serde_json::json!(true));
    reduced.insert(
        "runtime_field_count".to_string(),
        serde_json::json!(runtime_fields.len()),
    );
    for (field, value) in runtime_fields {
        if field.ends_with("_count") || field == "active_srdef_count" {
            reduced.insert(field.clone(), value.clone());
        }
    }
    reduced.insert(
        "budget_breach_codes".to_string(),
        serde_json::json!(["runtime_context_budget_count_only"]),
    );
    reduced
}

fn runtime_hash(projection: &AcpRuntimeContextProjection) -> String {
    let hash_input = serde_json::json!({
        "schema_version": projection.schema_version,
        "pack_id": projection.pack_id,
        "snapshot_id": projection.snapshot_id,
        "snapshot_created_at": projection.snapshot_created_at,
        "source_refs": projection.source_refs,
        "redaction_policy": projection.redaction_policy,
        "freshness_policy": projection.freshness_policy,
        "runtime_fields": projection.runtime_fields,
        "blocked_field_codes": projection.blocked_field_codes,
        "diagnostics": projection.diagnostics,
    });
    sha256_hex(&serde_json::to_vec(&hash_input).expect("runtime hash input should serialize"))
}

fn projection_hash(projection: &AcpRuntimeContextProjection) -> String {
    let hash_input = serde_json::json!({
        "schema_version": projection.schema_version,
        "pack_id": projection.pack_id,
        "static_envelope_hash": projection.static_envelope_hash,
        "runtime_hash": projection.runtime_hash,
    });
    sha256_hex(&serde_json::to_vec(&hash_input).expect("projection hash input should serialize"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
