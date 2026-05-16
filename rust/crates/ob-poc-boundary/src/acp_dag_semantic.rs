//! Generic read-only ACP semantic routing over authored DAG/DSL verbs.
//!
//! This is deliberately not an execution path. It gives ACP clients a
//! structured, bounded interpretation for non task-specific DAG utterances so
//! they do not fall back to prose-only acknowledgements.

use dsl_core::config::loader::ConfigLoader;
use dsl_core::config::types::{HarmClass, VerbConfig};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;
use uuid::Uuid;

use crate::acp_pack_context_envelope_v2::{
    load_online_acp_pack_context_registry_state_v2, AcpPackContextRegistryLoadOptions,
    AcpPackContextRegistryStateV2, ACP_PACK_CONTEXT_ENVELOPE_V2_SCHEMA_VERSION,
    ACP_PACK_CONTEXT_REGISTRY_STATE_V2_SCHEMA_VERSION,
};
use crate::acp_registry_projection::{
    build_slice1_acp_registry_projection, AcpRegistryProjection, SLICE_1_ACP_PACK_IDS,
};
use crate::acp_runtime_context::{build_acp_runtime_context_projection, AcpRuntimeContextSource};
use crate::pack_projection::{get_pack_projection_provider, PackProjection};

const MATCH_THRESHOLD: f32 = 0.42;
const AMBIGUITY_MARGIN: f32 = 0.08;
const PACK_MATCH_THRESHOLD: f32 = 0.48;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticResolution {
    pub status: AcpDagSemanticStatus,
    pub utterance: String,
    /// **R3 route trace v2:** kind-agnostic winner. Replaces the v1
    /// `selected_verb` field — the dispatch kind (verb vs macro vs
    /// pack template) is carried inline so consumers don't have to
    /// inspect three fields to know what was selected.
    pub selected_dispatch: Option<AcpDagSemanticSelectedDispatch>,
    /// Legacy field. Retained as a thin alias of
    /// `selected_dispatch.fqn` for the migration window only. New
    /// consumers MUST read `selected_dispatch`.
    pub selected_verb: Option<String>,
    pub selected_domain: Option<String>,
    pub selected_description: Option<String>,
    pub pack: Option<AcpDagSemanticPackContext>,
    pub selected_template: Option<AcpDagSemanticPackTemplate>,
    /// R3: candidate set including the winner. Carries `dispatch_kind`
    /// + `confidence_band` per v0.5 §17.3.
    pub top_candidates: Vec<AcpDagSemanticCandidate>,
    /// **R3 route trace v2:** rejected candidates with diagnostic codes
    /// per v0.5 §17.3. Lets HITL reviewers see *why* Sage picked X over
    /// Y from the persisted trace alone.
    pub rejected_candidates: Vec<AcpDagSemanticRejectedCandidate>,
    pub draft_dsl: Option<String>,
    pub workflow_plan: Option<AcpDagSemanticWorkflowPlan>,
    pub missing_required_args: Vec<String>,
    pub unresolved_refs: Vec<String>,
    pub read_only: bool,
    pub mutation_allowed: bool,
    pub requires_hitl: bool,
    pub structured_outcome_supported: bool,
    pub registry_trace: Option<AcpDagSemanticRegistryTrace>,
    pub envelope_trace: Option<AcpDagSemanticEnvelopeTrace>,
    pub runtime_trace: Option<AcpDagSemanticRuntimeTrace>,
    pub diagnostics: Vec<AcpDagSemanticDiagnostic>,
    /// **R8 single-path unification (2026-05-11):** ACP route metadata.
    /// Populated by the orchestrator when ACP resolution fires as the
    /// first step of `process()`. Carries the same route/latency/draft-
    /// source fields that the HTTP envelope used to attach via
    /// `annotate_acp_session_input_envelope`. The response adapter reads
    /// these into the flat `acp_trace` summary so the chat UI sees the
    /// same keys post-R8.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_metadata: Option<AcpRouteMetadata>,

    /// **R8 Phase B.7 (2026-05-11):** typed state-anchor provider trace.
    /// Populated when the ACP flow routed through a language-pack
    /// provider (KYC, deal). Carries the same fields the HTTP envelope
    /// previously produced under `result.observability.stateAnchorProvider`
    /// and `envelope.state_anchor_provider`, projected into the chat
    /// UI's flat `acp_trace.state_anchor_provider` sub-object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_anchor_provider: Option<AcpStateAnchorProvider>,

    /// **R8 Phase B.7 (2026-05-11):** observability summary (typed
    /// mirror of the HTTP envelope's `result.observability.conversationEfficiency`
    /// block). Phase A's prebuilt path read these from the JSON envelope;
    /// Phase B.7 lifts them onto the typed resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observability: Option<AcpObservabilitySummary>,

    /// **R8 Phase B.7 (2026-05-11):** override status when the
    /// language-pack flow produced an outcome the resolver's three-
    /// variant enum (`Refused`/`Ambiguous`/`Matched`) doesn't cover.
    /// Today's HTTP envelope can emit `dry_run_validated` from the
    /// deal language pack dry-run flow; that's stored here so
    /// `acp_chat_trace_summary_typed` can override the default status
    /// derivation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_status: Option<String>,
}

/// R8 Phase B.7 (2026-05-11): typed state-anchor provider trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpStateAnchorProvider {
    pub provider_selected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_pack_boundary: Option<String>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_anchor_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<String>,
    #[serde(default)]
    pub supported_tasks: Vec<String>,
    #[serde(default)]
    pub needed: Vec<String>,
    pub language_pack_generated: bool,
    pub dry_run_valid: bool,
    pub structured_outcome: bool,
    pub no_mutation_authority: bool,
}

/// R8 Phase B.7 (2026-05-11): observability summary for the chat-trace
/// projection. Sourced from the HTTP envelope's
/// `result.observability.conversationEfficiency` block today; future
/// slices will compute these typed at the orchestrator boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpObservabilitySummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_failure_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prose_only_failure: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_user_turn_required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_user_repair_turns_avoided: Option<u64>,
    /// Language-pack-produced transition reference (e.g.
    /// `deal.prospect-to-qualifying`). Pre-R8 came from
    /// `result.traceProjection.transitionRef` or
    /// `result.output.dry_run.transition_ref` in the HTTP envelope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_ref: Option<String>,
}

impl AcpDagSemanticResolution {
    /// R8 single-path unification (2026-05-11): typed equivalent of the
    /// HTTP-side `acp_valid_dag_semantic_draft_dsl(envelope)` predicate.
    ///
    /// Returns the draft DSL string only when the resolution is a
    /// `dag_semantic_proposal` (Matched status + draft_dsl present +
    /// no missing required args + no unresolved refs). Mirrors the
    /// `first_pass_valid_dsl_draft` predicate at
    /// `acp_protocol.rs::dag_semantic_outgoing`.
    pub fn first_pass_valid_draft_dsl(&self) -> Option<&str> {
        if self.status == AcpDagSemanticStatus::Matched
            && self.draft_dsl.is_some()
            && self.missing_required_args.is_empty()
            && self.unresolved_refs.is_empty()
        {
            self.draft_dsl.as_deref()
        } else {
            None
        }
    }
}

/// R8 Phase B (2026-05-11): typed mirror of the chat-UI `acp_trace`
/// summary previously built from a JSON envelope in
/// `agent_routes::acp_chat_trace_summary`. Reads typed fields off the
/// resolution + `route_metadata` and produces the same flat key shape
/// the frontend's `AcpTraceCard` expects.
///
/// Fields that today's HTTP path computes from HTTP-only envelope
/// blocks (`traceProjection.*`, `observability.*`) are emitted as
/// `null` until the orchestrator computes typed equivalents. The chat
/// UI treats these as optional.
pub fn acp_chat_trace_summary_typed(resolution: &AcpDagSemanticResolution) -> serde_json::Value {
    // R8 Phase B.7: prefer the override status when the language-pack
    // flow produced an outcome the resolver's three-variant enum
    // doesn't cover (e.g. `dry_run_validated` from the deal pack).
    let status_str: String = if let Some(ref override_status) = resolution.override_status {
        override_status.clone()
    } else {
        match resolution.status {
            AcpDagSemanticStatus::Refused => "structured_refusal".to_string(),
            AcpDagSemanticStatus::Ambiguous => "pending_question".to_string(),
            AcpDagSemanticStatus::Matched => {
                if resolution.first_pass_valid_draft_dsl().is_some() {
                    "dag_semantic_proposal".to_string()
                } else {
                    "pending_question".to_string()
                }
            }
        }
    };

    let outcome_layer = if status_str == "dry_run_validated" {
        Some("language_loop")
    } else {
        match resolution.status {
            AcpDagSemanticStatus::Refused => Some("structured_refusal"),
            AcpDagSemanticStatus::Ambiguous => Some("pending_question"),
            AcpDagSemanticStatus::Matched => Some("language_loop"),
        }
    };

    let route_metadata = resolution.route_metadata.as_ref();

    let refusal_code = if resolution.status == AcpDagSemanticStatus::Refused {
        resolution.diagnostics.first().map(|d| d.error_code.clone())
    } else {
        None
    };

    let pending_question_code = if resolution.status == AcpDagSemanticStatus::Ambiguous
        || (resolution.status == AcpDagSemanticStatus::Matched
            && resolution.first_pass_valid_draft_dsl().is_none())
    {
        resolution.diagnostics.first().map(|d| d.error_code.clone())
    } else {
        None
    };

    let candidate_verbs: Vec<String> = resolution
        .top_candidates
        .iter()
        .map(|c| c.fqn.clone())
        .collect();

    let candidate_verbs_value = if candidate_verbs.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Array(
            candidate_verbs
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        )
    };

    let diagnostic_codes: Vec<String> = resolution
        .diagnostics
        .iter()
        .map(|d| d.error_code.clone())
        .collect();
    let diagnostic_codes_value = if diagnostic_codes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Array(
            diagnostic_codes
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        )
    };

    let needed_from_user_value = if resolution.missing_required_args.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Array(
            resolution
                .missing_required_args
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        )
    };

    let pack_ref = resolution
        .pack
        .as_ref()
        .map(|p| format!("{}@{}", p.pack_id, p.pack_version));

    let workflow_plan_needed_from_user = resolution
        .workflow_plan
        .as_ref()
        .map(|plan| plan.needed_from_user.clone())
        .filter(|v| !v.is_empty());

    let selected_macro_id = resolution.selected_verb.as_ref().and_then(|selected| {
        resolution.top_candidates.iter().find_map(|c| {
            if &c.fqn == selected && c.side_effects.as_deref() == Some("macro_projection_only") {
                Some(c.fqn.clone())
            } else {
                None
            }
        })
    });

    let performance = route_metadata.map(|rm| {
        serde_json::json!({
            "total_ms": rm.route_latency_ms,
            "llm_draft_ms": 0u64,
        })
    });

    // Build via serde_json::Map to avoid the recursion limit of the
    // `json!` macro at ~40+ keys.
    use serde_json::{Map, Value};
    let mut summary: Map<String, Value> = Map::new();
    let mut put = |key: &str, value: Value| {
        summary.insert(key.to_string(), value);
    };
    let opt_str = |v: Option<String>| -> Value { v.map(Value::String).unwrap_or(Value::Null) };
    let opt_u64 =
        |v: Option<u64>| -> Value { v.map(|n| Value::Number(n.into())).unwrap_or(Value::Null) };
    let opt_bool = |v: Option<bool>| -> Value { v.map(Value::Bool).unwrap_or(Value::Null) };

    put("status", Value::String(status_str.clone()));
    put("outcome", Value::String(status_str.clone()));
    put("route", opt_str(route_metadata.map(|rm| rm.route.clone())));
    put(
        "provider_task",
        opt_str(route_metadata.map(|rm| rm.provider_task.clone())),
    );
    put(
        "requested_draft_source",
        opt_str(route_metadata.map(|rm| rm.requested_draft_source.clone())),
    );
    put(
        "draft_source",
        opt_str(route_metadata.map(|rm| rm.effective_draft_source.clone())),
    );
    put(
        "route_latency_ms",
        opt_u64(route_metadata.map(|rm| rm.route_latency_ms)),
    );
    put(
        "route_latency_us",
        opt_u64(route_metadata.map(|rm| rm.route_latency_us)),
    );
    put("outcome_layer", opt_str(outcome_layer.map(str::to_string)));
    put(
        "human_summary",
        Value::String(crate::acp_protocol::dag_semantic_human_message(resolution)),
    );
    put("prompt_context_variant", Value::Null);
    put(
        "transition_ref",
        opt_str(
            resolution
                .observability
                .as_ref()
                .and_then(|o| o.transition_ref.clone())
                .or_else(|| resolution.workflow_plan.as_ref().map(|p| p.plan_id.clone()))
                .or_else(|| {
                    resolution
                        .selected_template
                        .as_ref()
                        .map(|t| t.template_id.clone())
                }),
        ),
    );
    put("semantic_diff_uri", Value::Null);
    put("refusal_code", opt_str(refusal_code));
    put("pending_question_code", opt_str(pending_question_code));
    put("selected_verb", opt_str(resolution.selected_verb.clone()));
    put(
        "selected_dispatch_kind",
        resolution
            .selected_dispatch
            .as_ref()
            .map(|d| serde_json::to_value(d.dispatch_kind).unwrap_or(Value::Null))
            .unwrap_or(Value::Null),
    );
    put(
        "selected_dispatch_fqn",
        opt_str(resolution.selected_dispatch.as_ref().map(|d| d.fqn.clone())),
    );
    put(
        "selected_dispatch_confidence_band",
        resolution
            .selected_dispatch
            .as_ref()
            .map(|d| serde_json::to_value(d.confidence_band).unwrap_or(Value::Null))
            .unwrap_or(Value::Null),
    );
    put(
        "rejected_candidate_count",
        Value::Number((resolution.rejected_candidates.len() as u64).into()),
    );
    put(
        "pack_id",
        opt_str(resolution.pack.as_ref().map(|p| p.pack_id.clone())),
    );
    put(
        "pack_name",
        opt_str(resolution.pack.as_ref().map(|p| p.pack_name.clone())),
    );
    put("pack_ref", opt_str(pack_ref));
    put(
        "pack_allowed_verb_count",
        opt_u64(
            resolution
                .pack
                .as_ref()
                .map(|p| p.allowed_verb_count as u64),
        ),
    );
    put("candidate_verbs", candidate_verbs_value);
    put(
        "workflow_plan_id",
        opt_str(resolution.workflow_plan.as_ref().map(|p| p.plan_id.clone())),
    );
    put(
        "workflow_plan_verb",
        opt_str(resolution.workflow_plan.as_ref().map(|p| p.verb.clone())),
    );
    put(
        "workflow_plan_dry_run_only",
        opt_bool(resolution.workflow_plan.as_ref().map(|p| p.dry_run_only)),
    );
    put(
        "workflow_plan_needed_from_user",
        workflow_plan_needed_from_user
            .map(|v| Value::Array(v.into_iter().map(Value::String).collect()))
            .unwrap_or(Value::Null),
    );
    put(
        "selected_template_id",
        opt_str(
            resolution
                .selected_template
                .as_ref()
                .map(|t| t.template_id.clone()),
        ),
    );
    put(
        "structured_failure_mode",
        opt_str(
            resolution
                .observability
                .as_ref()
                .and_then(|o| o.structured_failure_mode.clone()),
        ),
    );
    put("needed_from_user", needed_from_user_value);
    put("diagnostic_codes", diagnostic_codes_value);
    put(
        "dry_run_valid",
        Value::Bool(
            resolution.status == AcpDagSemanticStatus::Matched
                && resolution.first_pass_valid_draft_dsl().is_some(),
        ),
    );
    put(
        "first_pass_valid",
        Value::Bool(resolution.first_pass_valid_draft_dsl().is_some()),
    );
    put(
        "revision_count",
        opt_u64(
            resolution
                .observability
                .as_ref()
                .and_then(|o| o.revision_count),
        ),
    );
    put(
        "prose_only_failure",
        opt_bool(
            resolution
                .observability
                .as_ref()
                .and_then(|o| o.prose_only_failure),
        ),
    );
    put(
        "pending_user_turn_required",
        opt_bool(
            resolution
                .observability
                .as_ref()
                .and_then(|o| o.pending_user_turn_required),
        ),
    );
    put(
        "estimated_user_repair_turns_avoided",
        opt_u64(
            resolution
                .observability
                .as_ref()
                .and_then(|o| o.estimated_user_repair_turns_avoided),
        ),
    );
    put("performance", performance.unwrap_or(Value::Null));
    put(
        "state_anchor_provider",
        resolution
            .state_anchor_provider
            .as_ref()
            .map(|p| serde_json::to_value(p).unwrap_or(Value::Null))
            .unwrap_or(Value::Null),
    );

    // Registry trace.
    put(
        "registry_schema_version",
        opt_str(
            resolution
                .registry_trace
                .as_ref()
                .map(|t| t.schema_version.clone()),
        ),
    );
    put(
        "registry_projection_hash",
        opt_str(
            resolution
                .registry_trace
                .as_ref()
                .map(|t| t.source_projection_hash.clone()),
        ),
    );
    put(
        "registry_verified",
        opt_bool(resolution.registry_trace.as_ref().map(|t| t.verified)),
    );

    // Envelope trace.
    put(
        "envelope_schema_version",
        opt_str(
            resolution
                .envelope_trace
                .as_ref()
                .map(|t| t.schema_version.clone()),
        ),
    );
    put(
        "envelope_hash",
        opt_str(
            resolution
                .envelope_trace
                .as_ref()
                .map(|t| t.envelope_hash.clone()),
        ),
    );
    put(
        "envelope_pack_id",
        opt_str(
            resolution
                .envelope_trace
                .as_ref()
                .map(|t| t.pack_id.clone()),
        ),
    );
    put(
        "projection_hash",
        opt_str(
            resolution
                .envelope_trace
                .as_ref()
                .map(|t| t.source_projection_hash.clone())
                .or_else(|| {
                    resolution
                        .registry_trace
                        .as_ref()
                        .map(|t| t.source_projection_hash.clone())
                }),
        ),
    );
    put(
        "envelope_verified",
        opt_bool(resolution.envelope_trace.as_ref().map(|t| t.verified)),
    );

    // Runtime trace.
    put(
        "runtime_schema_version",
        opt_str(
            resolution
                .runtime_trace
                .as_ref()
                .map(|t| t.schema_version.clone()),
        ),
    );
    put(
        "runtime_pack_id",
        opt_str(resolution.runtime_trace.as_ref().map(|t| t.pack_id.clone())),
    );
    put(
        "runtime_snapshot_id",
        opt_str(
            resolution
                .runtime_trace
                .as_ref()
                .map(|t| t.snapshot_id.clone()),
        ),
    );
    put(
        "runtime_hash",
        opt_str(
            resolution
                .runtime_trace
                .as_ref()
                .map(|t| t.runtime_hash.clone()),
        ),
    );
    put(
        "runtime_redaction_policy",
        opt_str(
            resolution
                .runtime_trace
                .as_ref()
                .map(|t| t.redaction_policy.clone()),
        ),
    );
    put(
        "runtime_freshness_policy",
        opt_str(
            resolution
                .runtime_trace
                .as_ref()
                .map(|t| t.freshness_policy.clone()),
        ),
    );
    put(
        "runtime_static_envelope_hash",
        opt_str(
            resolution
                .runtime_trace
                .as_ref()
                .map(|t| t.static_envelope_hash.clone()),
        ),
    );
    put(
        "runtime_projection_hash",
        opt_str(
            resolution
                .runtime_trace
                .as_ref()
                .map(|t| t.projection_hash.clone()),
        ),
    );
    put(
        "runtime_verified",
        opt_bool(resolution.runtime_trace.as_ref().map(|t| t.verified)),
    );
    put(
        "runtime_redacted_count",
        opt_u64(
            resolution
                .runtime_trace
                .as_ref()
                .map(|t| t.redacted_count as u64),
        ),
    );
    put(
        "runtime_blocked_field_codes",
        resolution
            .runtime_trace
            .as_ref()
            .map(|t| t.blocked_field_codes.clone())
            .filter(|v| !v.is_empty())
            .map(|v| Value::Array(v.into_iter().map(Value::String).collect()))
            .unwrap_or(Value::Null),
    );
    put("selected_macro_id", opt_str(selected_macro_id));

    Value::Object(summary)
}

/// R8 single-path unification (2026-05-11): route-layer metadata
/// attached to an `AcpDagSemanticResolution` when the orchestrator
/// resolved the ACP step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpRouteMetadata {
    /// Route discriminator. `"session_input"` for the unified session
    /// pipeline path. Other values may appear if additional routes
    /// adopt the typed metadata in the future.
    pub route: String,
    /// Provider task tag, e.g. `"kyc-case.update-status"`. Resolved
    /// from `acp_prompt_supported_provider_task(&prompt)` or falls back
    /// to `"dag.semantic"`.
    pub provider_task: String,
    /// Draft mode the orchestrator was configured to attempt.
    pub requested_draft_source: String,
    /// Draft mode that actually ran (may differ on LLM→deterministic
    /// fallback).
    pub effective_draft_source: String,
    /// End-to-end resolution latency in microseconds.
    pub route_latency_us: u64,
    /// Same latency, rounded up to milliseconds. Mirrored from the
    /// pre-R8 envelope shape so existing chat UI consumers keep the
    /// same key.
    pub route_latency_ms: u64,
}

/// R3 — kind-agnostic dispatch winner. Replaces `selected_verb`.
///
/// The agent reads `dispatch_kind` once to know whether the winner was a
/// verb, a macro, or a pack template; all three share the same outer
/// shape so consumers don't branch on field presence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticSelectedDispatch {
    pub dispatch_kind: AcpDagSemanticDispatchKind,
    pub fqn: String,
    pub confidence: f32,
    pub confidence_band: AcpDagSemanticConfidenceBand,
    pub matched_phrase: Option<String>,
    pub description: Option<String>,
}

/// Dispatch kind discriminator for the route trace.
///
/// Mirrors `AcpDslAtomKind` from the envelope projection, plus a
/// `Template` variant for pack-local workbook-plan templates that
/// haven't yet been lifted to registry-grade macros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpDagSemanticDispatchKind {
    Verb,
    Macro,
    Template,
}

/// Confidence band for human-readable diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpDagSemanticConfidenceBand {
    /// score ≥ 0.85
    High,
    /// 0.60 ≤ score < 0.85
    Medium,
    /// 0.40 ≤ score < 0.60 — Sage may ask a clarifying question
    Low,
    /// score < 0.40 — refusal or pending question expected
    BelowThreshold,
}

impl AcpDagSemanticConfidenceBand {
    pub fn from_score(score: f32) -> Self {
        if score >= 0.85 {
            Self::High
        } else if score >= 0.60 {
            Self::Medium
        } else if score >= 0.40 {
            Self::Low
        } else {
            Self::BelowThreshold
        }
    }
}

/// R3 — rejected candidate with diagnostic code.
///
/// Carries enough information for HITL reviewers to answer "why X
/// instead of Y" without re-running the resolver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticRejectedCandidate {
    pub dispatch_kind: AcpDagSemanticDispatchKind,
    pub fqn: String,
    pub score: f32,
    pub confidence_band: AcpDagSemanticConfidenceBand,
    /// Diagnostic code from the projected taxonomy: `ambiguous_pack`,
    /// `forbidden_verb`, `missing_binding`, `unsupported_macro_tier`,
    /// `legacy_route_bait`, `below_match_threshold`, `lost_to_higher_scorer`.
    pub rejection_code: String,
    /// Short human-readable rationale (audit-grade).
    pub rejection_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticRegistryTrace {
    pub schema_version: String,
    pub source_projection_hash: String,
    pub pack_count: usize,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticEnvelopeTrace {
    pub schema_version: String,
    pub registry_schema_version: String,
    pub pack_id: String,
    pub lifecycle: String,
    pub envelope_hash: String,
    pub source_projection_hash: String,
    pub section_hash_count: usize,
    pub content_hash_chain_count: usize,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticRuntimeTrace {
    pub schema_version: String,
    pub pack_id: String,
    pub snapshot_id: String,
    pub runtime_hash: String,
    pub redaction_policy: String,
    pub freshness_policy: String,
    pub static_envelope_hash: String,
    pub projection_hash: String,
    pub verified: bool,
    pub redacted_count: usize,
    pub blocked_field_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticPackContext {
    pub pack_id: String,
    pub pack_name: String,
    pub pack_version: String,
    pub pack_hash: String,
    pub score: f32,
    pub matched_phrase: Option<String>,
    pub description: String,
    pub invocation_phrases: Vec<String>,
    pub workspaces: Vec<String>,
    pub required_context: Vec<String>,
    pub optional_context: Vec<String>,
    pub allowed_verbs: Vec<String>,
    pub allowed_verb_count: usize,
    pub forbidden_verbs: Vec<String>,
    pub risk_policy: AcpDagSemanticPackRiskPolicy,
    pub required_questions: Vec<AcpDagSemanticPackQuestion>,
    pub optional_questions: Vec<AcpDagSemanticPackQuestion>,
    pub stop_rules: Vec<String>,
    pub templates: Vec<AcpDagSemanticPackTemplate>,
    pub pack_summary_template: Option<String>,
    pub section_layout: Vec<AcpDagSemanticPackSection>,
    pub definition_of_done: Vec<String>,
    pub progress_signals: Vec<AcpDagSemanticPackProgressSignal>,
    pub handoff_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticPackRiskPolicy {
    pub require_confirm_before_execute: bool,
    pub max_steps_without_confirm: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticPackSection {
    pub title: String,
    pub verb_prefixes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticPackQuestion {
    pub field: String,
    pub prompt: String,
    pub answer_kind: String,
    pub options_source: Option<String>,
    pub default: Option<serde_json::Value>,
    pub ask_when: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticPackTemplate {
    pub template_id: String,
    pub when_to_use: String,
    pub steps: Vec<AcpDagSemanticPackTemplateStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticPackTemplateStep {
    pub verb: String,
    pub args: BTreeMap<String, serde_json::Value>,
    pub repeat_for: Option<String>,
    pub when: Option<String>,
    pub execution_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticPackProgressSignal {
    pub signal: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpDagSemanticStatus {
    Matched,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticCandidate {
    /// **R3:** kind discriminator. The resolver currently only scores
    /// verbs (this is the existing pre-R3 behaviour). Macro and template
    /// scoring ride on R2a's `dsl_atoms` and land in a follow-up slice;
    /// R3 ships the schema so the wire shape is stable.
    pub dispatch_kind: AcpDagSemanticDispatchKind,
    /// Kind-agnostic FQN (verb FQN, macro FQN, or pack template id).
    /// Renamed from the v1 `verb` field per the kind-agnostic discipline.
    pub fqn: String,
    pub domain: String,
    pub score: f32,
    /// **R3:** confidence band for human-readable diagnostics.
    pub confidence_band: AcpDagSemanticConfidenceBand,
    pub read_only: bool,
    pub harm_class: String,
    pub side_effects: Option<String>,
    pub required_args: Vec<String>,
    pub matched_phrase: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticDiagnostic {
    pub error_code: String,
    pub source: String,
    pub message: String,
    pub expected: Vec<String>,
    pub actual: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticWorkflowPlan {
    pub plan_id: String,
    pub verb: String,
    pub objective: String,
    pub dry_run_only: bool,
    pub mutation_allowed: bool,
    pub requires_hitl: bool,
    pub input_bindings: Vec<AcpDagSemanticWorkflowBinding>,
    pub read_model: Vec<String>,
    pub would_create_or_update: Vec<String>,
    pub state_transitions: Vec<AcpDagSemanticWorkflowTransition>,
    pub dictionary_projection: AcpDagSemanticDictionaryProjection,
    pub blocked_reasons: Vec<String>,
    pub needed_from_user: Vec<String>,
    pub context_requirements: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticWorkflowBinding {
    pub field: String,
    pub required: bool,
    pub binding_status: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticWorkflowTransition {
    pub entity: String,
    pub from: String,
    pub to: String,
    pub verb: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpDagSemanticDictionaryProjection {
    pub grain: String,
    pub slice_rule: String,
    pub attr_rule: String,
    pub owner_dispatch_rule: String,
    pub completion_rule: String,
}

#[derive(Debug, Clone)]
struct AcpDagVerbRow {
    fqn: String,
    domain: String,
    verb_name: String,
    description: String,
    phrases: Vec<String>,
    required_args: Vec<String>,
    read_only: bool,
    harm_class: String,
    side_effects: Option<String>,
}

#[derive(Debug, Clone)]
struct ScoredRow {
    row: AcpDagVerbRow,
    score: f32,
    matched_phrase: Option<String>,
}

#[derive(Debug, Clone)]
struct AcpDagSemanticIndex {
    rows: Vec<AcpDagVerbRow>,
    packs: Vec<AcpDagPackRow>,
}

#[derive(Debug, Clone)]
struct AcpDagPackRow {
    /// Boundary-owned projection of one pack from the catalogue. The
    /// integrator's projection function (ob-poc) is the only place that
    /// knows how to derive this from the upstream `PackManifest`; this
    /// crate only reads.
    projection: PackProjection,
}

#[derive(Debug, Clone)]
struct ScoredPack {
    row: AcpDagPackRow,
    score: f32,
    matched_phrase: Option<String>,
}

pub fn resolve_acp_dag_semantic_prompt(
    utterance: &str,
) -> Result<Option<AcpDagSemanticResolution>, String> {
    let utterance = utterance.trim();
    if utterance.is_empty() {
        return Ok(None);
    }
    if looks_like_acp_control_prompt(utterance) {
        return Ok(None);
    }

    let index = semantic_index()?;
    if let Some(refusal) = classify_structured_refusal(index, utterance) {
        return Ok(Some(refusal));
    }

    let pack = select_pack(index, utterance);
    let scored = score_rows(index, utterance, pack.as_ref());

    let Some(top) = scored.first().cloned() else {
        return Ok(None);
    };
    let ambiguous = scored
        .get(1)
        .map(|runner_up| top.score - runner_up.score < AMBIGUITY_MARGIN)
        .unwrap_or(false);
    let disambiguated = ambiguous
        .then(|| disambiguated_candidate(&scored, pack.as_ref(), utterance))
        .flatten();

    let status = if ambiguous && disambiguated.is_none() {
        AcpDagSemanticStatus::Ambiguous
    } else {
        AcpDagSemanticStatus::Matched
    };
    let selected = disambiguated
        .map(|candidate| candidate.row)
        .or_else(|| (status == AcpDagSemanticStatus::Matched).then_some(top.row.clone()));
    let (draft_dsl, missing_required_args, unresolved_refs) = selected
        .as_ref()
        .map(|row| draft_dsl_for_row(row, utterance))
        .unwrap_or((None, Vec::new(), Vec::new()));
    let workflow_plan = selected
        .as_ref()
        .and_then(|row| workflow_plan_for_row(row, utterance, &missing_required_args));
    let selected_pack = pack.or_else(|| {
        selected
            .as_ref()
            .and_then(|row| infer_slice_pack_for_selected_verb(index, utterance, row))
    });
    let selected_template = selected_pack
        .as_ref()
        .zip(selected.as_ref())
        .and_then(|(pack, row)| template_context_for_selected_verb(pack, row));

    let mut diagnostics = Vec::new();
    if ambiguous && status == AcpDagSemanticStatus::Ambiguous {
        diagnostics.push(AcpDagSemanticDiagnostic {
            error_code: "dag_semantic_ambiguous_verb".to_string(),
            source: "acp_dag_semantic_router".to_string(),
            message: "Multiple authored DSL verbs match this utterance closely".to_string(),
            expected: scored.iter().map(|row| row.row.fqn.clone()).collect(),
            actual: None,
        });
    }
    if !missing_required_args.is_empty() {
        diagnostics.push(AcpDagSemanticDiagnostic {
            error_code: "dag_semantic_missing_required_args".to_string(),
            source: "acp_dag_semantic_router".to_string(),
            message: "The selected DSL verb requires additional argument bindings".to_string(),
            expected: missing_required_args.clone(),
            actual: None,
        });
    }

    let read_only = selected.as_ref().map(|row| row.read_only).unwrap_or(false);
    let selected_fqn = selected.as_ref().map(|row| row.fqn.clone());
    let candidates: Vec<AcpDagSemanticCandidate> =
        scored.into_iter().map(candidate_from_scored).collect();
    // R3: look up the winner's confidence + matched_phrase from the
    // scored candidate set (the resolver returns a bare row from the
    // (dis)ambiguation step; the scored entry carries score/phrase).
    let selected_dispatch = selected.as_ref().map(|row| {
        let winner_candidate = candidates.iter().find(|c| c.fqn == row.fqn);
        let score = winner_candidate.map(|c| c.score).unwrap_or(1.0);
        AcpDagSemanticSelectedDispatch {
            dispatch_kind: AcpDagSemanticDispatchKind::Verb,
            fqn: row.fqn.clone(),
            confidence: score,
            confidence_band: AcpDagSemanticConfidenceBand::from_score(score),
            matched_phrase: winner_candidate.and_then(|c| c.matched_phrase.clone()),
            description: Some(row.description.clone()),
        }
    });
    // R3: split the scored list into winner-retained `top_candidates` and
    // structured `rejected_candidates` so HITL reviewers can answer
    // "why X over Y" from the trace alone.
    let rejected_candidates = build_rejected_candidates(&candidates, selected_fqn.as_deref());
    Ok(Some(AcpDagSemanticResolution {
        status,
        utterance: utterance.to_string(),
        selected_verb: selected_fqn,
        selected_dispatch,
        selected_domain: selected.as_ref().map(|row| row.domain.clone()),
        selected_description: selected.as_ref().map(|row| row.description.clone()),
        pack: selected_pack.as_ref().map(pack_context_from_scored),
        selected_template,
        top_candidates: candidates,
        rejected_candidates,
        draft_dsl,
        workflow_plan,
        missing_required_args,
        unresolved_refs,
        read_only,
        mutation_allowed: false,
        requires_hitl: !read_only,
        structured_outcome_supported: true,
        registry_trace: None,
        envelope_trace: None,
        runtime_trace: None,
        diagnostics,
        route_metadata: None,
        state_anchor_provider: None,
        observability: None,
        override_status: None,
    }))
}

/// Build rejected-candidate trace entries with diagnostic codes.
///
/// Each non-winning candidate is tagged with one of the diagnostic
/// taxonomy codes the envelope projects (`lost_to_higher_scorer`,
/// `below_match_threshold`). Future iterations can layer in
/// `forbidden_verb`, `missing_binding`, etc. when those gates fail.
fn build_rejected_candidates(
    candidates: &[AcpDagSemanticCandidate],
    winner_fqn: Option<&str>,
) -> Vec<AcpDagSemanticRejectedCandidate> {
    candidates
        .iter()
        .filter(|c| Some(c.fqn.as_str()) != winner_fqn)
        .map(|c| {
            let (code, reason) = if c.score < PACK_MATCH_THRESHOLD {
                (
                    "below_match_threshold".to_string(),
                    format!(
                        "score {:.2} below match threshold {:.2}",
                        c.score, PACK_MATCH_THRESHOLD
                    ),
                )
            } else {
                (
                    "lost_to_higher_scorer".to_string(),
                    format!("scored {:.2}; outranked by the winning candidate", c.score),
                )
            };
            AcpDagSemanticRejectedCandidate {
                dispatch_kind: c.dispatch_kind,
                fqn: c.fqn.clone(),
                score: c.score,
                confidence_band: c.confidence_band,
                rejection_code: code,
                rejection_reason: reason,
            }
        })
        .collect()
}

/// Resolve an ACP DAG semantic utterance with verified Slice 1 envelope context.
///
/// The base router still handles non-Slice packs, but any Slice 1 pack selected
/// by the resolution must be backed by a verified active envelope from the
/// development online registry state. No-pack structured refusals receive a
/// registry trace so callers can prove the refusal still passed through the
/// verified registry boundary.
///
/// # Examples
///
/// ```rust,no_run
/// use ob_poc_boundary::acp_dag_semantic::resolve_acp_dag_semantic_prompt_with_verified_envelopes;
///
/// let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
/// let resolution = resolve_acp_dag_semantic_prompt_with_verified_envelopes(
///     "assign role to cbu",
///     config_root,
/// )
/// .unwrap()
/// .unwrap();
/// assert!(resolution.registry_trace.is_some());
/// ```
pub fn resolve_acp_dag_semantic_prompt_with_verified_envelopes(
    utterance: &str,
    config_root: impl AsRef<Path>,
) -> Result<Option<AcpDagSemanticResolution>, String> {
    let Some(mut resolution) = resolve_acp_dag_semantic_prompt(utterance)? else {
        return Ok(None);
    };
    let config_root = config_root.as_ref();
    let projection = build_slice1_acp_registry_projection(config_root)
        .map_err(|error| format!("building Slice 1 registry projection failed: {error}"))?;
    let registry_state = load_online_acp_pack_context_registry_state_v2(
        &projection,
        config_root,
        AcpPackContextRegistryLoadOptions::development(),
    )
    .map_err(|refusal| {
        format!(
            "loading verified ACP pack context registry failed: {} ({})",
            refusal.code, refusal.message
        )
    })?;
    attach_verified_envelope_trace(&mut resolution, &projection, &registry_state);
    Ok(Some(resolution))
}

fn disambiguated_candidate(
    scored: &[ScoredRow],
    pack: Option<&ScoredPack>,
    utterance: &str,
) -> Option<ScoredRow> {
    if pack
        .map(|pack| pack.row.projection.indexing.id.as_str() != "cbu-maintenance")
        .unwrap_or(false)
    {
        return None;
    }
    let normalized = normalize_text(utterance);
    let utterance_tokens = tokens(&normalized);
    let cbu_product_signal = has_any(&utterance_tokens, CBU_PRODUCT_TERMS)
        && has_any(&utterance_tokens, &["cbu", "fund"])
        && has_any(
            &utterance_tokens,
            &[
                "add",
                "assign",
                "attach",
                "enable",
                "link",
                "onboard",
                "onboarding",
                "subscribe",
                "activate",
                "provision",
            ],
        )
        && !has_any(
            &utterance_tokens,
            &[
                "delete",
                "disable",
                "remove",
                "unsubscribe",
                "deactivate",
                "purge",
            ],
        );
    if !cbu_product_signal {
        return None;
    }
    scored
        .iter()
        .find(|candidate| candidate.row.fqn == "cbu.add-product")
        .cloned()
}

fn classify_structured_refusal(
    index: &AcpDagSemanticIndex,
    utterance: &str,
) -> Option<AcpDagSemanticResolution> {
    let lower = utterance.to_ascii_lowercase();
    let normalized = normalize_text(utterance);
    let utterance_tokens = tokens(&normalized);

    if lower.contains("raw dsl") || normalized.contains("direct dsl") {
        return Some(refused_resolution(
            utterance,
            None,
            None,
            "dag_semantic_refused_direct_dsl_bypass",
            "Direct DSL bypass requests are not valid utterance routes",
            vec!["POST /api/session/:id/input with kind=utterance".to_string()],
            Some("raw_or_direct_dsl_bypass".to_string()),
        ));
    }
    if normalized.contains("legacy execute endpoint") {
        return Some(refused_resolution(
            utterance,
            None,
            None,
            "dag_semantic_refused_legacy_execute_route",
            "Legacy execute endpoint requests are excluded from normal utterance routing",
            vec!["POST /api/session/:id/input with kind=utterance".to_string()],
            Some("legacy_execute_endpoint".to_string()),
        ));
    }
    if normalized.contains("legacy pipeline") {
        return Some(refused_resolution(
            utterance,
            None,
            None,
            "dag_semantic_refused_legacy_pipeline_route",
            "Legacy pipeline fallback requests are excluded from Slice 1 utterance routing",
            vec!["verified pack-scoped route".to_string()],
            Some("legacy_pipeline".to_string()),
        ));
    }
    if normalized.contains("old chat route") {
        return Some(refused_resolution(
            utterance,
            None,
            None,
            "dag_semantic_refused_removed_chat_route",
            "The old chat route is removed and cannot be used as an utterance path",
            vec!["POST /api/session/:id/input with kind=utterance".to_string()],
            Some("removed_chat_route".to_string()),
        ));
    }
    if normalized.contains("bypass pack filtering") || normalized.contains("bypass") {
        return Some(refused_resolution(
            utterance,
            None,
            None,
            "dag_semantic_refused_pack_filter_bypass",
            "Bypassing pack filtering is not allowed",
            vec!["pack-scoped routing".to_string()],
            Some("pack_filter_bypass".to_string()),
        ));
    }

    if has_any(&utterance_tokens, &["delete", "deleting"])
        && (has_any(&utterance_tokens, &["cbu", "fund"]) || normalized.contains("delete it"))
    {
        return Some(refused_resolution_for_pack(
            index,
            utterance,
            "cbu-maintenance",
            "cbu.delete",
            "dag_semantic_refused_forbidden_pack_verb",
            "The CBU Maintenance pack forbids destructive CBU deletion from utterance routing",
        ));
    }
    if ((has_any(
        &utterance_tokens,
        &["provision", "provisioning", "activate"],
    ) || normalized.contains("provision the service resource"))
        && normalized.contains("service resource"))
        || normalized.contains("delete every product service resource")
    {
        return Some(refused_resolution_for_pack(
            index,
            utterance,
            "product-service-taxonomy",
            "service-resource.provision",
            "dag_semantic_refused_forbidden_pack_verb",
            "The Product Service Taxonomy pack is read-oriented and forbids service-resource provisioning/destruction from utterance routing",
        ));
    }
    if normalized.contains("onboarding dispatch")
        && (has_any(&utterance_tokens, &["execute", "dispatch", "run"])
            || normalized.contains("without owner approval"))
    {
        return Some(refused_resolution_for_pack(
            index,
            utterance,
            "onboarding-request",
            "onboarding.dispatch-ready-slices",
            "dag_semantic_refused_hitl_required",
            "Onboarding dispatch requires owner approval and cannot be executed from this utterance",
        ));
    }

    None
}

fn refused_resolution_for_pack(
    index: &AcpDagSemanticIndex,
    utterance: &str,
    pack_id: &str,
    verb: &str,
    error_code: &str,
    message: &str,
) -> AcpDagSemanticResolution {
    refused_resolution(
        utterance,
        pack_context_by_id(index, pack_id),
        row_by_fqn(index, verb),
        error_code,
        message,
        vec![verb.to_string()],
        Some(verb.to_string()),
    )
}

fn refused_resolution(
    utterance: &str,
    pack: Option<AcpDagSemanticPackContext>,
    row: Option<AcpDagVerbRow>,
    error_code: &str,
    message: &str,
    expected: Vec<String>,
    actual: Option<String>,
) -> AcpDagSemanticResolution {
    let selected_verb = actual.clone();
    let selected_dispatch = selected_verb
        .as_ref()
        .map(|fqn| AcpDagSemanticSelectedDispatch {
            dispatch_kind: AcpDagSemanticDispatchKind::Verb,
            fqn: fqn.clone(),
            confidence: 1.0,
            confidence_band: AcpDagSemanticConfidenceBand::High,
            matched_phrase: None,
            description: row.as_ref().map(|row| row.description.clone()),
        });
    AcpDagSemanticResolution {
        status: AcpDagSemanticStatus::Refused,
        utterance: utterance.to_string(),
        selected_verb,
        selected_dispatch,
        selected_domain: row.as_ref().map(|row| row.domain.clone()),
        selected_description: row.as_ref().map(|row| row.description.clone()),
        pack,
        selected_template: None,
        top_candidates: row
            .map(|row| AcpDagSemanticCandidate {
                dispatch_kind: AcpDagSemanticDispatchKind::Verb,
                fqn: row.fqn,
                domain: row.domain,
                score: 1.0,
                confidence_band: AcpDagSemanticConfidenceBand::High,
                read_only: row.read_only,
                harm_class: row.harm_class,
                side_effects: row.side_effects,
                required_args: row.required_args,
                matched_phrase: None,
                description: row.description,
            })
            .into_iter()
            .collect(),
        rejected_candidates: Vec::new(),
        draft_dsl: None,
        workflow_plan: None,
        missing_required_args: Vec::new(),
        unresolved_refs: Vec::new(),
        read_only: true,
        mutation_allowed: false,
        requires_hitl: false,
        structured_outcome_supported: true,
        registry_trace: None,
        envelope_trace: None,
        runtime_trace: None,
        diagnostics: vec![AcpDagSemanticDiagnostic {
            error_code: error_code.to_string(),
            source: "acp_dag_semantic_router".to_string(),
            message: message.to_string(),
            expected,
            actual,
        }],
        route_metadata: None,
        state_anchor_provider: None,
        observability: None,
        override_status: None,
    }
}

fn attach_verified_envelope_trace(
    resolution: &mut AcpDagSemanticResolution,
    projection: &AcpRegistryProjection,
    registry_state: &AcpPackContextRegistryStateV2,
) {
    resolution.registry_trace = Some(AcpDagSemanticRegistryTrace {
        schema_version: registry_state.schema_version.clone(),
        source_projection_hash: registry_state.source_projection_hash.clone(),
        pack_count: registry_state.pack_count,
        verified: registry_state.source_projection_hash == projection.projection_hash
            && registry_state.schema_version == ACP_PACK_CONTEXT_REGISTRY_STATE_V2_SCHEMA_VERSION,
    });

    let Some(pack) = resolution.pack.as_ref() else {
        return;
    };
    if !SLICE_1_ACP_PACK_IDS.contains(&pack.pack_id.as_str()) {
        return;
    }
    let Some(envelope) = registry_state
        .envelopes
        .iter()
        .find(|envelope| envelope.body.pack_id == pack.pack_id)
    else {
        resolution.diagnostics.push(AcpDagSemanticDiagnostic {
            error_code: "dag_semantic_verified_envelope_missing".to_string(),
            source: "acp_dag_semantic_envelope_gate".to_string(),
            message: "Selected Slice 1 pack is not backed by an active verified envelope"
                .to_string(),
            expected: vec![pack.pack_id.clone()],
            actual: None,
        });
        return;
    };
    resolution.envelope_trace = Some(AcpDagSemanticEnvelopeTrace {
        schema_version: envelope.schema_version.clone(),
        registry_schema_version: registry_state.schema_version.clone(),
        pack_id: envelope.body.pack_id.clone(),
        lifecycle: format!("{:?}", envelope.body.lifecycle).to_ascii_lowercase(),
        envelope_hash: envelope.envelope_hash.clone(),
        source_projection_hash: envelope.body.build_inputs.source_projection_hash.clone(),
        section_hash_count: envelope.body.section_hashes.len(),
        content_hash_chain_count: envelope.body.content_hash_chain.len(),
        verified: envelope.schema_version == ACP_PACK_CONTEXT_ENVELOPE_V2_SCHEMA_VERSION
            && envelope.body.build_inputs.source_projection_hash == projection.projection_hash,
    });
    attach_runtime_context_trace(resolution);
}

fn attach_runtime_context_trace(resolution: &mut AcpDagSemanticResolution) {
    let (Some(pack), Some(envelope_trace)) =
        (resolution.pack.as_ref(), resolution.envelope_trace.as_ref())
    else {
        return;
    };
    let mut fields = BTreeMap::new();
    fields.insert(
        "binding_status".to_string(),
        serde_json::json!("request_scoped"),
    );
    if !resolution.missing_required_args.is_empty() {
        fields.insert(
            "missing_binding_codes".to_string(),
            serde_json::json!(resolution.missing_required_args),
        );
    }
    if let Some(workflow_plan) = &resolution.workflow_plan {
        fields.insert(
            "workbook_step_statuses".to_string(),
            serde_json::json!(workflow_plan
                .state_transitions
                .iter()
                .enumerate()
                .map(|(index, transition)| serde_json::json!({
                    "step_id": format!("{}:{}", index + 1, transition.verb),
                    "status": "planned"
                }))
                .collect::<Vec<_>>()),
        );
    }

    let selected_ref = resolution
        .selected_verb
        .as_deref()
        .or_else(|| {
            resolution
                .selected_template
                .as_ref()
                .map(|template| template.template_id.as_str())
        })
        .unwrap_or("unknown");
    let snapshot_id = format!(
        "dag-semantic-runtime:{}",
        stable_runtime_context_hash(&format!(
            "{}:{}:{}:{}",
            pack.pack_id, selected_ref, resolution.utterance, envelope_trace.envelope_hash
        ))
    );
    let runtime_projection = build_acp_runtime_context_projection(AcpRuntimeContextSource {
        pack_id: pack.pack_id.clone(),
        session_id: None,
        snapshot_id,
        snapshot_created_at: "request_scoped".to_string(),
        source_refs: vec![
            format!("dag_semantic:pack:{}", pack.pack_id),
            format!("dag_semantic:selected:{selected_ref}"),
        ],
        static_envelope_hash: envelope_trace.envelope_hash.clone(),
        fields,
        stale: false,
        missing_source_codes: Vec::new(),
        force_count_only: false,
        field_budget: None,
    });

    resolution.runtime_trace = Some(AcpDagSemanticRuntimeTrace {
        schema_version: runtime_projection.schema_version,
        pack_id: runtime_projection.pack_id,
        snapshot_id: runtime_projection.snapshot_id,
        runtime_hash: runtime_projection.runtime_hash,
        redaction_policy: runtime_projection.redaction_policy,
        freshness_policy: runtime_projection.freshness_policy,
        static_envelope_hash: runtime_projection.static_envelope_hash,
        projection_hash: runtime_projection.projection_hash,
        verified: runtime_projection.verified,
        redacted_count: runtime_projection.redacted_count,
        blocked_field_codes: runtime_projection.blocked_field_codes,
    });
}

fn stable_runtime_context_hash(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(input.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn pack_context_by_id(
    index: &AcpDagSemanticIndex,
    pack_id: &str,
) -> Option<AcpDagSemanticPackContext> {
    index
        .packs
        .iter()
        .find(|pack| pack.projection.indexing.id == pack_id)
        .map(|row| {
            pack_context_from_scored(&ScoredPack {
                row: row.clone(),
                score: 1.0,
                matched_phrase: None,
            })
        })
}

fn row_by_fqn(index: &AcpDagSemanticIndex, fqn: &str) -> Option<AcpDagVerbRow> {
    index.rows.iter().find(|row| row.fqn == fqn).cloned()
}

fn semantic_index() -> Result<&'static AcpDagSemanticIndex, String> {
    #[cfg(test)]
    crate::pack_projection::ensure_test_provider_registered();

    static INDEX: OnceLock<Result<AcpDagSemanticIndex, String>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            let config = ConfigLoader::from_env()
                .load_verbs()
                .map_err(|error| error.to_string())?;
            let mut rows = Vec::new();
            for (domain, domain_config) in config.domains {
                for (verb_name, verb_config) in domain_config.verbs {
                    rows.push(row_from_config(&domain, &verb_name, &verb_config));
                }
            }
            rows.extend(slice_macro_rows());
            // The pack catalogue is fetched from the boundary-owned
            // provider (registered by the ob-poc integrator at startup
            // — see pack_projection.rs). Boundary no longer reaches
            // into ob-poc-journey for PackManifest or for the disk
            // loader. The catalogue's eventual home is SemOS-via-MCP.
            let provider = get_pack_projection_provider().map_err(str::to_string)?;
            let packs = provider()?.into_iter().map(row_from_projection).collect();
            Ok(AcpDagSemanticIndex { rows, packs })
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn row_from_projection(projection: PackProjection) -> AcpDagPackRow {
    AcpDagPackRow { projection }
}

fn score_rows(
    index: &AcpDagSemanticIndex,
    utterance: &str,
    pack: Option<&ScoredPack>,
) -> Vec<ScoredRow> {
    let pack_allowed_verbs = pack.map(|pack| &pack.row.projection.indexing.allowed_verbs);
    let mut scored = index
        .rows
        .iter()
        .filter_map(|row| score_row(row, utterance, pack_allowed_verbs))
        .filter(|row| row.score >= MATCH_THRESHOLD)
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.row.fqn.cmp(&right.row.fqn))
    });
    scored.truncate(5);
    scored
}

fn row_from_config(domain: &str, verb_name: &str, config: &VerbConfig) -> AcpDagVerbRow {
    let fqn = format!("{domain}.{verb_name}");
    let side_effects = config
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.side_effects.clone());
    let harm_class = config
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.harm_class)
        .unwrap_or_else(|| infer_harm_class(verb_name, config));
    let read_only =
        matches!(harm_class, HarmClass::ReadOnly) || side_effects.as_deref() == Some("facts_only");

    let mut phrases = BTreeSet::new();
    phrases.insert(fqn.clone());
    let bare_verb_phrase = verb_name.replace('-', " ");
    if !is_generic_bare_action_phrase(&bare_verb_phrase) {
        phrases.insert(bare_verb_phrase);
    }
    phrases.insert(format!(
        "{} {}",
        domain.replace('-', " "),
        verb_name.replace('-', " ")
    ));
    phrases.insert(config.description.clone());
    for phrase in &config.invocation_phrases {
        phrases.insert(phrase.clone());
    }

    AcpDagVerbRow {
        fqn,
        domain: domain.to_string(),
        verb_name: verb_name.to_string(),
        description: config.description.clone(),
        phrases: phrases.into_iter().collect(),
        required_args: config
            .args
            .iter()
            .filter(|arg| arg.required)
            .map(|arg| arg.name.clone())
            .collect(),
        read_only,
        harm_class: format!("{harm_class:?}"),
        side_effects,
    }
}

fn slice_macro_rows() -> Vec<AcpDagVerbRow> {
    vec![
        AcpDagVerbRow {
            fqn: "struct.lux.ucits.sicav".to_string(),
            domain: "struct".to_string(),
            verb_name: "lux-ucits-sicav".to_string(),
            description: "Luxembourg UCITS SICAV structure macro".to_string(),
            phrases: vec![
                "struct.lux.ucits.sicav".to_string(),
                "luxembourg ucits sicav".to_string(),
                "luxembourg ucits sicav structure".to_string(),
                "set up luxembourg ucits sicav structure".to_string(),
            ],
            required_args: Vec::new(),
            read_only: true,
            harm_class: "ReadOnly".to_string(),
            side_effects: Some("macro_projection_only".to_string()),
        },
        AcpDagVerbRow {
            fqn: "structure.product-suite-custody-fa-ta".to_string(),
            domain: "structure".to_string(),
            verb_name: "product-suite-custody-fa-ta".to_string(),
            description: "Custody, fund accounting, and transfer agency product-suite macro"
                .to_string(),
            phrases: vec![
                "structure.product-suite-custody-fa-ta".to_string(),
                "custody fa ta product suite".to_string(),
                "full custody fa ta product suite".to_string(),
                "create full custody fa ta product suite".to_string(),
            ],
            required_args: Vec::new(),
            read_only: true,
            harm_class: "ReadOnly".to_string(),
            side_effects: Some("macro_projection_only".to_string()),
        },
    ]
}

fn infer_harm_class(verb_name: &str, config: &VerbConfig) -> HarmClass {
    let normalized = verb_name.to_ascii_lowercase();
    if config
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.side_effects.as_deref())
        == Some("facts_only")
    {
        return HarmClass::ReadOnly;
    }
    if normalized.starts_with("list")
        || normalized.starts_with("read")
        || normalized.starts_with("show")
        || normalized.starts_with("get")
        || normalized.starts_with("describe")
    {
        return HarmClass::ReadOnly;
    }
    if normalized.contains("delete") || normalized.contains("purge") {
        return HarmClass::Irreversible;
    }
    HarmClass::Reversible
}

fn select_pack(index: &AcpDagSemanticIndex, utterance: &str) -> Option<ScoredPack> {
    let normalized_utterance = normalize_text(utterance);
    let utterance_tokens = tokens(&normalized_utterance);
    if utterance_tokens.is_empty() {
        return None;
    }

    let mut scored = index
        .packs
        .iter()
        .filter_map(|row| score_pack(row, &normalized_utterance, &utterance_tokens))
        .filter(|pack| pack.score >= PACK_MATCH_THRESHOLD)
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right.score.total_cmp(&left.score).then_with(|| {
            left.row
                .projection
                .indexing
                .id
                .cmp(&right.row.projection.indexing.id)
        })
    });
    let top = scored.first()?.clone();
    let clear_pack = scored
        .get(1)
        .map(|runner_up| top.score - runner_up.score >= 0.12)
        .unwrap_or(true);
    clear_pack.then_some(top)
}

fn infer_slice_pack_for_selected_verb(
    index: &AcpDagSemanticIndex,
    utterance: &str,
    selected: &AcpDagVerbRow,
) -> Option<ScoredPack> {
    let normalized_utterance = normalize_text(utterance);
    let utterance_tokens = tokens(&normalized_utterance);
    let target_pack_ids = [
        "onboarding-request",
        "cbu-maintenance",
        "product-service-taxonomy",
    ];
    let mut candidates = index
        .packs
        .iter()
        .filter(|pack| target_pack_ids.contains(&pack.projection.indexing.id.as_str()))
        .filter(|pack| {
            pack.projection
                .indexing
                .allowed_verbs
                .contains(&selected.fqn)
        })
        .filter_map(|pack| {
            slice_pack_signal_score(
                &pack.projection.indexing.id,
                &normalized_utterance,
                &utterance_tokens,
            )
            .map(|score| ScoredPack {
                row: pack.clone(),
                score,
                matched_phrase: Some(format!("selected verb {}", selected.fqn)),
            })
        })
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        candidates = index
            .packs
            .iter()
            .filter(|pack| target_pack_ids.contains(&pack.projection.indexing.id.as_str()))
            .filter(|pack| {
                pack.projection
                    .indexing
                    .allowed_verbs
                    .contains(&selected.fqn)
            })
            .map(|pack| ScoredPack {
                row: pack.clone(),
                score: 0.6,
                matched_phrase: Some(format!("selected verb {}", selected.fqn)),
            })
            .collect::<Vec<_>>();
    }

    candidates.sort_by(|left, right| {
        right.score.total_cmp(&left.score).then_with(|| {
            left.row
                .projection
                .indexing
                .id
                .cmp(&right.row.projection.indexing.id)
        })
    });
    let top = candidates.first()?.clone();
    let clear_pack = candidates
        .get(1)
        .map(|runner_up| top.score - runner_up.score >= 0.12)
        .unwrap_or(true);
    clear_pack.then_some(top)
}

fn slice_pack_signal_score(pack_id: &str, normalized: &str, tokens: &[String]) -> Option<f32> {
    match pack_id {
        "onboarding-request"
            if normalized.contains("onboarding request status")
                || normalized.contains("onboarding workbook plan")
                || (has_any(tokens, &["onboarding"])
                    && has_any(
                        tokens,
                        &["continue", "current", "plan", "status", "workbook"],
                    )) =>
        {
            Some(1.25)
        }
        "cbu-maintenance"
            if (has_any(tokens, &["cbu"]) || normalized.contains("client business unit"))
                && (normalized.contains("product binding status")
                    || (has_any(tokens, &["product"])
                        && has_any(tokens, &["binding", "status"]))) =>
        {
            Some(1.25)
        }
        "product-service-taxonomy"
            if normalized.contains("discovered service resource details")
                || normalized.contains("missing service resource")
                || (has_any(tokens, &["service"])
                    && has_any(tokens, &["resource"])
                    && has_any(tokens, &["details", "discovered", "missing"])) =>
        {
            Some(1.25)
        }
        "onboarding-request"
            if has_any(
                tokens,
                &[
                    "contract",
                    "data",
                    "deal",
                    "handoff",
                    "onboarding",
                    "request",
                    "slice",
                    "slices",
                ],
            ) =>
        {
            Some(0.82)
        }
        "cbu-maintenance"
            if has_any(
                tokens,
                &[
                    "cbu", "client", "fund", "product", "resource", "role", "unit",
                ],
            ) || normalized.contains("client business unit") =>
        {
            Some(0.82)
        }
        "product-service-taxonomy"
            if has_any(
                tokens,
                &[
                    "attribute",
                    "attributes",
                    "product",
                    "resource",
                    "service",
                    "taxonomy",
                    "version",
                ],
            ) =>
        {
            Some(0.82)
        }
        _ => None,
    }
}

fn score_pack(
    row: &AcpDagPackRow,
    normalized_utterance: &str,
    utterance_tokens: &[String],
) -> Option<ScoredPack> {
    let mut best_score = 0.0_f32;
    let mut matched_phrase = None;
    for phrase in &row.projection.indexing.phrases {
        let normalized_phrase = normalize_text(phrase);
        let phrase_tokens = tokens(&normalized_phrase);
        if phrase_tokens.is_empty() {
            continue;
        }

        let mut score = token_overlap_score(utterance_tokens, &phrase_tokens);
        if normalized_utterance == normalized_phrase {
            score += 1.0;
        } else if normalized_utterance.contains(&normalized_phrase)
            || normalized_phrase.contains(normalized_utterance)
        {
            score += 0.65;
        }
        if score > best_score {
            best_score = score;
            matched_phrase = Some(phrase.clone());
        }
    }
    if let Some(slice_score) = slice_pack_signal_score(
        &row.projection.indexing.id,
        normalized_utterance,
        utterance_tokens,
    ) {
        if slice_score > best_score {
            best_score = slice_score;
            matched_phrase = Some("slice pack signal".to_string());
        }
    }

    (best_score > 0.0).then(|| ScoredPack {
        row: row.clone(),
        score: (best_score * 1000.0).round() / 1000.0,
        matched_phrase,
    })
}

fn score_row(
    row: &AcpDagVerbRow,
    utterance: &str,
    pack_allowed_verbs: Option<&BTreeSet<String>>,
) -> Option<ScoredRow> {
    let normalized_utterance = normalize_text(utterance);
    let utterance_tokens = tokens(&normalized_utterance);
    if utterance_tokens.is_empty() {
        return None;
    }

    let mut best_score = 0.0_f32;
    let mut matched_phrase = None;
    for phrase in &row.phrases {
        let normalized_phrase = normalize_text(phrase);
        let phrase_tokens = tokens(&normalized_phrase);
        if phrase_tokens.is_empty() {
            continue;
        }

        let mut score = token_overlap_score(&utterance_tokens, &phrase_tokens);
        if normalized_utterance == normalized_phrase {
            score += 1.0;
        } else if normalized_utterance.contains(&normalized_phrase)
            || normalized_phrase.contains(&normalized_utterance)
        {
            score += 0.55;
        }
        if score > best_score {
            best_score = score;
            matched_phrase = Some(phrase.clone());
        }
    }

    best_score += domain_boost(row, &utterance_tokens, &normalized_utterance);
    best_score += action_boost(row, &utterance_tokens);
    best_score += cbu_role_boost(row, &utterance_tokens, &normalized_utterance);
    best_score += cbu_create_boost(row, &utterance_tokens, &normalized_utterance);
    best_score += cbu_bulk_create_boost(row, &utterance_tokens, &normalized_utterance);
    best_score += onboarding_data_request_boost(row, &utterance_tokens, &normalized_utterance);
    best_score += product_service_taxonomy_boost(row, &utterance_tokens, &normalized_utterance);
    if let Some(allowed_verbs) = pack_allowed_verbs {
        if allowed_verbs.contains(&row.fqn) {
            best_score += 0.55;
        } else {
            best_score *= 0.35;
        }
    }

    (best_score > 0.0).then(|| ScoredRow {
        row: row.clone(),
        score: (best_score * 1000.0).round() / 1000.0,
        matched_phrase,
    })
}

fn pack_context_from_scored(scored: &ScoredPack) -> AcpDagSemanticPackContext {
    // The context is pre-projected by the ob-poc integrator at startup
    // (see crate::pack_projection). We just clone it and patch in the
    // per-utterance score + matched phrase.
    let mut context = scored.row.projection.context.clone();
    context.score = scored.score;
    context.matched_phrase = scored.matched_phrase.clone();
    context
}

fn template_context_for_selected_verb(
    scored: &ScoredPack,
    selected: &AcpDagVerbRow,
) -> Option<AcpDagSemanticPackTemplate> {
    scored
        .row
        .projection
        .context
        .templates
        .iter()
        .find(|template| template.steps.iter().any(|step| step.verb == selected.fqn))
        .cloned()
}

/// Project a `crate::session::WorkspaceKind` to its boundary-canonical
/// string form. Exposed `pub` so the ob-poc integrator's pack-projection
/// function can render workspace names in the same shape boundary stores
/// in `AcpDagSemanticPackContext::workspaces`.
pub fn workspace_context_name(workspace: &crate::session::WorkspaceKind) -> String {
    serde_json::to_value(workspace)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{workspace:?}").to_ascii_lowercase())
}

fn token_overlap_score(utterance_tokens: &[String], phrase_tokens: &[String]) -> f32 {
    let utterance = utterance_tokens.iter().collect::<BTreeSet<_>>();
    let phrase = phrase_tokens.iter().collect::<BTreeSet<_>>();
    let common = utterance.intersection(&phrase).count() as f32;
    if common == 0.0 {
        return 0.0;
    }
    let min_len = utterance.len().min(phrase.len()).max(1) as f32;
    let max_len = utterance.len().max(phrase.len()).max(1) as f32;
    (common / min_len) * 0.45 + (common / max_len) * 0.25
}

fn domain_boost(row: &AcpDagVerbRow, tokens: &[String], normalized: &str) -> f32 {
    let aliases = domain_aliases(&row.domain);
    if aliases.iter().any(|alias| {
        tokens.iter().any(|token| token == alias) || normalized.contains(&alias.replace('-', " "))
    }) {
        0.2
    } else {
        0.0
    }
}

fn action_boost(row: &AcpDagVerbRow, utterance_tokens: &[String]) -> f32 {
    let verb_tokens = tokens(&normalize_text(&row.verb_name));
    let mut boost = 0.0_f32;
    for action in normalized_actions(utterance_tokens) {
        if verb_tokens.iter().any(|token| token == &action) {
            boost += 0.16;
        }
    }
    boost.min(0.32)
}

fn cbu_role_boost(row: &AcpDagVerbRow, tokens: &[String], normalized: &str) -> f32 {
    if row.fqn == "cbu.assign-role"
        && (has_cbu_role_term(tokens) || has_any_phrase(normalized, CBU_ROLE_PHRASES))
        && has_any(
            tokens,
            &["add", "assign", "appoint", "make", "link", "connect"],
        )
    {
        return 1.05;
    }
    if row.fqn == "cbu.remove-role"
        && (has_cbu_role_term(tokens) || has_any_phrase(normalized, CBU_ROLE_PHRASES))
        && has_any(
            tokens,
            &["delete", "remove", "unassign", "unlink", "detach", "revoke"],
        )
    {
        return 1.05;
    }
    if row.fqn == "cbu.parties"
        && (normalized.contains("who are")
            || normalized.contains("who is")
            || has_any(tokens, CBU_PARTY_READ_TERMS))
    {
        return 0.9;
    }
    if row.fqn == "cbu.add-product"
        && (has_any(tokens, &["cbu"]) || normalized.contains("client business unit"))
        && (normalized.contains("product binding status")
            || (has_any(tokens, &["product"]) && has_any(tokens, &["binding", "status"])))
    {
        return 1.05;
    }
    if row.fqn == "cbu.add-product"
        && has_any(tokens, CBU_PRODUCT_TERMS)
        && has_any(
            tokens,
            &[
                "add",
                "assign",
                "attach",
                "enable",
                "link",
                "onboard",
                "subscribe",
                "activate",
                "provision",
                "set",
            ],
        )
    {
        return 0.9;
    }
    if row.fqn == "cbu.remove-product"
        && has_any(tokens, CBU_PRODUCT_TERMS)
        && has_any(
            tokens,
            &["delete", "disable", "remove", "unsubscribe", "deactivate"],
        )
    {
        return 0.9;
    }
    if row.fqn == "cbu.delete-cascade"
        && has_any(tokens, &["cbu", "fund"])
        && (has_any(tokens, &["cascade", "completely", "purge"])
            || normalized.contains("all related"))
    {
        return 1.15;
    }
    0.0
}

fn cbu_create_boost(row: &AcpDagVerbRow, tokens: &[String], normalized: &str) -> f32 {
    if row.fqn == "cbu.create"
        && (has_any(tokens, &["cbu", "fund"]) || normalized.contains("client business unit"))
        && has_any(
            tokens,
            &["create", "new", "open", "register", "set", "setup", "start"],
        )
    {
        return 1.2;
    }
    0.0
}

fn cbu_bulk_create_boost(row: &AcpDagVerbRow, tokens: &[String], normalized: &str) -> f32 {
    if row.fqn != "cbu.create-from-client-group" {
        return 0.0;
    }
    let has_bulk_intent = has_any(
        tokens,
        &[
            "allianz",
            "batch",
            "bulk",
            "convert",
            "generate",
            "gleif",
            "mass",
            "onboard",
            "onboarding",
            "open",
            "research",
            "start",
        ],
    );
    let has_client_group_source = normalized.contains("client group")
        || has_any(
            tokens,
            &[
                "cbus", "cbu", "client", "entities", "entity", "fund", "funds", "group",
            ],
        );
    if has_bulk_intent && has_client_group_source {
        0.95
    } else {
        0.0
    }
}

fn product_service_taxonomy_boost(row: &AcpDagVerbRow, tokens: &[String], normalized: &str) -> f32 {
    if row.fqn == "product.list"
        && normalized.contains("product taxonomy")
        && has_any(tokens, &["show", "list", "view"])
    {
        return 1.15;
    }
    if row.fqn == "service-resource.list-by-service"
        && normalized.contains("service resource map")
        && has_any(tokens, &["service", "resource"])
    {
        return 1.15;
    }
    if row.fqn == "service-resource.list-attributes"
        && (normalized.contains("resource dictionary")
            || normalized.contains("discovered service resource details")
            || normalized.contains("missing service resource")
            || (has_any(tokens, &["service"])
                && has_any(tokens, &["resource"])
                && has_any(tokens, &["details", "discovered", "missing"])))
    {
        return 1.15;
    }
    0.0
}

fn onboarding_data_request_boost(row: &AcpDagVerbRow, tokens: &[String], normalized: &str) -> f32 {
    if row.fqn == "onboarding.compile-data-request"
        && (normalized.contains("data request")
            || normalized.contains("data dictionary")
            || normalized.contains("onboarding workbook plan")
            || normalized.contains("resource dictionary")
            || normalized.contains("resource requirements")
            || normalized.contains("owner data request")
            || (has_any(
                tokens,
                &["compile", "freeze", "prepare", "build", "consolidate"],
            ) && has_any(
                tokens,
                &["onboarding", "resource", "requirements", "dictionary"],
            )))
    {
        return 1.15;
    }
    if row.fqn == "onboarding.dispatch-ready-slices"
        && has_any(tokens, &["dispatch", "send", "release", "forward"])
        && has_any(tokens, &["slice", "slices", "owner", "owners", "ready"])
    {
        return 1.0;
    }
    if row.fqn == "onboarding.get-data-request"
        && has_any(tokens, &["get", "show", "read"])
        && normalized.contains("data request")
    {
        return 0.8;
    }
    if row.fqn == "onboarding.cancel-data-request"
        && has_any(tokens, &["cancel", "stop", "void"])
        && normalized.contains("data request")
    {
        return 1.1;
    }
    if row.fqn == "onboarding.list-slices"
        && has_any(tokens, &["list", "show"])
        && has_any(tokens, &["slice", "slices", "owner", "owners"])
    {
        return 0.8;
    }
    if row.fqn == "onboarding.list-data-requests"
        && normalized.contains("onboarding request status")
    {
        return 1.1;
    }
    if row.fqn == "onboarding.compile-data-request"
        && normalized.contains("product onboarding requirements")
    {
        return 1.1;
    }
    0.0
}

const CBU_ROLE_TERMS: &[&str] = &[
    "administrator",
    "auditor",
    "custodian",
    "depositary",
    "director",
    "entity",
    "gp",
    "im",
    "lp",
    "manco",
    "manager",
    "owner",
    "participant",
    "party",
    "role",
    "shareholder",
    "signatory",
    "ubo",
];

const CBU_ROLE_PHRASES: &[&str] = &[
    "authorized signatory",
    "beneficial owner",
    "general partner",
    "investment manager",
    "limited partner",
    "management company",
    "prime broker",
    "transfer agent",
];

const CBU_PARTY_READ_TERMS: &[&str] = &[
    "directors",
    "participants",
    "parties",
    "roles",
    "roster",
    "signatories",
    "stakeholders",
    "ubos",
];

const CBU_PRODUCT_TERMS: &[&str] = &[
    "accounting",
    "custody",
    "nav",
    "product",
    "service",
    "ta",
    "trading",
];

fn has_any(tokens: &[String], values: &[&str]) -> bool {
    values
        .iter()
        .any(|value| tokens.iter().any(|token| token == value))
}

fn has_any_phrase(normalized: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|phrase| normalized.contains(phrase))
}

fn has_cbu_role_term(tokens: &[String]) -> bool {
    has_any(tokens, CBU_ROLE_TERMS)
}

fn is_generic_bare_action_phrase(phrase: &str) -> bool {
    matches!(
        phrase,
        "add"
            | "assign"
            | "create"
            | "delete"
            | "get"
            | "list"
            | "read"
            | "remove"
            | "set"
            | "show"
            | "update"
    )
}

fn normalized_actions(tokens: &[String]) -> Vec<String> {
    tokens
        .iter()
        .map(|token| match token.as_str() {
            "appoint" | "appointed" | "make" | "link" | "connect" => "assign".to_string(),
            "add" | "create" | "new" | "register" | "onboard" => "create".to_string(),
            "remove" | "delete" | "unlink" | "purge" => "delete".to_string(),
            "show" | "list" | "who" | "what" | "view" | "get" => "list".to_string(),
            "change" | "set" | "move" | "advance" | "transition" => "update".to_string(),
            other => other.to_string(),
        })
        .collect()
}

fn domain_aliases(domain: &str) -> Vec<String> {
    let mut aliases = vec![domain.to_ascii_lowercase(), domain.replace('-', " ")];
    match domain {
        "cbu" => aliases.extend(["fund", "funds", "client business unit"].map(str::to_string)),
        "kyc-case" => aliases.extend(["case", "kyc case"].map(str::to_string)),
        "deal" => aliases.extend(["deal", "opportunity"].map(str::to_string)),
        "entity" => aliases.extend(["entity", "company", "person"].map(str::to_string)),
        "document" => aliases.extend(["document", "documents", "doc"].map(str::to_string)),
        _ => {}
    }
    aliases
}

fn candidate_from_scored(scored: ScoredRow) -> AcpDagSemanticCandidate {
    let confidence_band = AcpDagSemanticConfidenceBand::from_score(scored.score);
    AcpDagSemanticCandidate {
        // R3: candidate kind is `Verb` for everything the resolver
        // currently scores. Macro/template scoring will populate the
        // other variants when those scorers wire in.
        dispatch_kind: AcpDagSemanticDispatchKind::Verb,
        fqn: scored.row.fqn,
        domain: scored.row.domain,
        score: scored.score,
        confidence_band,
        read_only: scored.row.read_only,
        harm_class: scored.row.harm_class,
        side_effects: scored.row.side_effects,
        required_args: scored.row.required_args,
        matched_phrase: scored.matched_phrase,
        description: scored.row.description,
    }
}

fn workflow_plan_for_row(
    row: &AcpDagVerbRow,
    utterance: &str,
    missing_required_args: &[String],
) -> Option<AcpDagSemanticWorkflowPlan> {
    match row.fqn.as_str() {
        "onboarding.compile-data-request" => Some(onboarding_compile_data_request_plan(
            row,
            utterance,
            missing_required_args,
        )),
        _ => None,
    }
}

fn onboarding_compile_data_request_plan(
    row: &AcpDagVerbRow,
    utterance: &str,
    missing_required_args: &[String],
) -> AcpDagSemanticWorkflowPlan {
    let input_bindings = row
        .required_args
        .iter()
        .map(|arg| {
            let value = infer_arg_value(arg, utterance);
            AcpDagSemanticWorkflowBinding {
                field: arg.clone(),
                required: true,
                binding_status: if value.is_some() {
                    "bound".to_string()
                } else {
                    "missing".to_string()
                },
                value,
            }
        })
        .collect::<Vec<_>>();
    let mut blocked_reasons = missing_required_args
        .iter()
        .map(|arg| format!("missing required binding `{arg}`"))
        .collect::<Vec<_>>();
    if missing_required_args.is_empty() {
        blocked_reasons.push(
            "exact slice and attribute counts require read-only database discovery for the onboarding request"
                .to_string(),
        );
    }

    AcpDagSemanticWorkflowPlan {
        plan_id: "onboarding.compile-data-request.preview.v1".to_string(),
        verb: row.fqn.clone(),
        objective: "Preview the frozen CBU-level service-resource data dictionary that would be compiled for one deal onboarding request".to_string(),
        dry_run_only: true,
        mutation_allowed: false,
        requires_hitl: true,
        input_bindings,
        read_model: vec![
            "deal_onboarding_requests: resolve deal_id, contract_id, cbu_id, product_id".to_string(),
            "srdef_discovery_reasons: active service-resource discoveries for the target CBU".to_string(),
            "service_resource_types: owner, provisioning strategy, L4 binding policy, SRDEF snapshot".to_string(),
            "service_resource_attribute_requirements: required attributes for each discovered SRDEF".to_string(),
            "cbu_attribute_values: existing CBU-scoped values used to mark attribute rows complete or missing".to_string(),
            "resource_owner_principals: dispatchable owners for each resource slice".to_string(),
        ],
        would_create_or_update: vec![
            "onboarding_data_requests: one frozen request header".to_string(),
            "onboarding_data_request_discoveries: immutable discovery snapshots".to_string(),
            "onboarding_data_request_slices: one slice per SRDEF/parameter/owner grouping".to_string(),
            "onboarding_data_request_attrs: one frozen requirement row per slice attribute".to_string(),
            "deal_onboarding_requests: request_status may advance from PENDING to IN_PROGRESS on execution".to_string(),
        ],
        state_transitions: vec![
            AcpDagSemanticWorkflowTransition {
                entity: "onboarding_data_request".to_string(),
                from: "collecting".to_string(),
                to: "ready_for_dispatch".to_string(),
                verb: "onboarding.compile-data-request".to_string(),
            },
            AcpDagSemanticWorkflowTransition {
                entity: "onboarding_data_request_slice".to_string(),
                from: "collecting".to_string(),
                to: "ready".to_string(),
                verb: "onboarding.compile-data-request".to_string(),
            },
            AcpDagSemanticWorkflowTransition {
                entity: "deal_onboarding_request".to_string(),
                from: "PENDING".to_string(),
                to: "IN_PROGRESS".to_string(),
                verb: "onboarding.compile-data-request".to_string(),
            },
        ],
        dictionary_projection: AcpDagSemanticDictionaryProjection {
            grain: "CBU x product x active service-resource discovery".to_string(),
            slice_rule: "one owner-addressable slice per active SRDEF discovery and parameter set".to_string(),
            attr_rule: "freeze each SRDEF attribute requirement with condition, evidence policy, constraints, current value status, and blocker".to_string(),
            owner_dispatch_rule: "ready slices can later be dispatched only where owner principal and L4 binding policy are satisfied".to_string(),
            completion_rule: "request completes when all non-cancelled slices are activated or otherwise terminal".to_string(),
        },
        needed_from_user: missing_required_args.to_vec(),
        blocked_reasons,
        context_requirements: vec![
            "onboarding-request-id must identify a deal_onboarding_request".to_string(),
            "that request must resolve to one existing CBU and one contracted product".to_string(),
            "CBU service-resource discovery must already have run or the dictionary will be empty".to_string(),
            "resource owners and application/L4 bindings determine dispatch readiness".to_string(),
        ],
    }
}

fn draft_dsl_for_row(
    row: &AcpDagVerbRow,
    utterance: &str,
) -> (Option<String>, Vec<String>, Vec<String>) {
    let mut missing = Vec::new();
    let mut unresolved = Vec::new();
    let mut args = BTreeMap::new();
    for arg in &row.required_args {
        match infer_arg_value(arg, utterance) {
            Some(value) => {
                args.insert(arg.clone(), value);
            }
            None => {
                missing.push(arg.clone());
                unresolved.push(format!("{arg}=<required>"));
                args.insert(arg.clone(), format!("<required:{arg}>"));
            }
        }
    }

    let arg_text = args
        .iter()
        .map(|(name, value)| format!(" :{name} {}", quote_dsl_value(value)))
        .collect::<String>();
    (
        Some(format!("({}{arg_text})", row.fqn)),
        missing,
        unresolved,
    )
}

fn infer_arg_value(arg: &str, utterance: &str) -> Option<String> {
    let lower = utterance.to_ascii_lowercase();
    if arg.ends_with("-id") || arg.ends_with("_id") || arg.contains("uuid") {
        if let Some(uuid) = extract_uuid(utterance) {
            return Some(uuid);
        }
        if let Some(symbolic_id) = extract_symbolic_id_for_arg(arg, utterance) {
            return Some(symbolic_id);
        }
    }
    if arg.contains("role") {
        if let Some(role) = infer_role(&lower) {
            return Some(role);
        }
    }
    if arg == "name" || arg.ends_with("-name") || arg.ends_with("_name") {
        if let Some(name) = extract_after_keyword(utterance, "called")
            .or_else(|| extract_after_keyword(utterance, "named"))
            .or_else(|| extract_after_keyword(utterance, "for"))
        {
            if is_plausible_name_binding(&name) {
                return Some(name);
            }
        }
    }
    None
}

fn is_plausible_name_binding(value: &str) -> bool {
    !matches!(
        normalize_text(value).as_str(),
        "approval" | "approvals" | "confirmation" | "confirmations" | "permission" | "permissions"
    )
}

fn extract_symbolic_id_for_arg(arg: &str, utterance: &str) -> Option<String> {
    let prefixes: &[&str] = if arg.contains("product") {
        &["P"]
    } else if arg.contains("service") {
        &["S"]
    } else if arg.contains("resource") {
        &["R", "SR"]
    } else if arg.contains("cbu") {
        &["C", "CBU"]
    } else if arg.contains("deal") {
        &["D", "DEAL"]
    } else {
        &[]
    };

    utterance
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | ',' | ';' | '(' | ')'))
        .find_map(|token| {
            let token = token.trim_matches(|ch: char| matches!(ch, ':' | '.' | '!' | '?'));
            let (prefix, suffix) = token.split_once('-')?;
            let prefix_matches = prefixes
                .iter()
                .any(|expected| prefix.eq_ignore_ascii_case(expected));
            (prefix_matches && !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
                .then(|| token.to_string())
        })
}

fn extract_uuid(value: &str) -> Option<String> {
    value
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | ',' | ';' | '(' | ')'))
        .find_map(|token| {
            let token = token.trim_matches(|ch: char| matches!(ch, ':' | '.' | '!' | '?'));
            Uuid::parse_str(token).ok().map(|uuid| uuid.to_string())
        })
}

fn infer_role(lower: &str) -> Option<String> {
    let roles = [
        ("director", "DIRECTOR"),
        ("beneficial owner", "BENEFICIAL_OWNER"),
        ("ubo", "BENEFICIAL_OWNER"),
        ("signatory", "SIGNATORY"),
        ("shareholder", "SHAREHOLDER"),
        ("manco", "MANAGEMENT_COMPANY"),
        ("management company", "MANAGEMENT_COMPANY"),
        ("depositary", "DEPOSITARY"),
        ("custodian", "CUSTODIAN"),
        ("auditor", "AUDITOR"),
        ("administrator", "ADMINISTRATOR"),
        ("transfer agent", "TRANSFER_AGENT"),
        ("prime broker", "PRIME_BROKER"),
        ("general partner", "GENERAL_PARTNER"),
        ("limited partner", "LIMITED_PARTNER"),
    ];
    roles
        .iter()
        .find_map(|(needle, role)| lower.contains(needle).then(|| (*role).to_string()))
}

fn extract_after_keyword(utterance: &str, keyword: &str) -> Option<String> {
    let lower = utterance.to_ascii_lowercase();
    let needle = format!(" {keyword} ");
    let start = lower.find(&needle).map(|idx| idx + needle.len())?;
    let value = utterance[start..].trim().trim_matches(['"', '\'']);
    (!value.is_empty()).then(|| value.to_string())
}

fn quote_dsl_value(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

fn normalize_text(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .replace(['_', '-', '.', '/', ':'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_acp_control_prompt(utterance: &str) -> bool {
    matches!(
        normalize_text(utterance).as_str(),
        "assemble context"
            | "build context"
            | "context"
            | "open context"
            | "show context"
            | "projection catalogue"
            | "show projections"
            | "list projections"
            | "hello"
            | "help"
            | "status"
    )
}

fn tokens(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .filter(|token| !STOP_WORDS.contains(token))
        .map(ToOwned::to_owned)
        .collect()
}

const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "for", "from", "in", "into", "is", "me", "of", "on", "or", "the",
    "this", "to", "with",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// R3 acceptance: every matched resolution carries a `selected_dispatch`
    /// with kind + confidence band, candidates carry kind + confidence
    /// band, and non-winners populate `rejected_candidates` with a
    /// diagnostic code.
    #[test]
    fn r3_route_trace_v2_populates_dispatch_and_rejected_candidates() {
        let resolved = resolve_acp_dag_semantic_prompt("assign role to cbu")
            .expect("resolver should not error")
            .expect("utterance should match");

        // selected_dispatch present + populated
        let dispatch = resolved
            .selected_dispatch
            .as_ref()
            .expect("selected_dispatch must be Some on matched resolution");
        assert_eq!(dispatch.dispatch_kind, AcpDagSemanticDispatchKind::Verb);
        assert!(!dispatch.fqn.is_empty());
        assert!(dispatch.confidence > 0.0);
        // legacy alias still populated for migration window
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some(dispatch.fqn.as_str())
        );

        // every candidate carries kind + confidence band
        for candidate in &resolved.top_candidates {
            assert_eq!(candidate.dispatch_kind, AcpDagSemanticDispatchKind::Verb);
            // band derives from score
            let derived = AcpDagSemanticConfidenceBand::from_score(candidate.score);
            assert_eq!(candidate.confidence_band, derived);
        }

        // rejected_candidates carry diagnostic codes — at least one
        // expected when top_candidates exceeds 1.
        if resolved.top_candidates.len() > 1 {
            assert!(
                !resolved.rejected_candidates.is_empty(),
                "expected rejected_candidates when scored set has multiple entries"
            );
            for rejected in &resolved.rejected_candidates {
                assert!(
                    matches!(
                        rejected.rejection_code.as_str(),
                        "lost_to_higher_scorer" | "below_match_threshold"
                    ),
                    "rejection_code {} not in expected set",
                    rejected.rejection_code,
                );
                assert!(!rejected.rejection_reason.is_empty());
                assert!(Some(rejected.fqn.as_str()) != Some(dispatch.fqn.as_str()));
            }
        }
    }

    #[test]
    fn r3_confidence_band_bucketing_is_correct() {
        assert_eq!(
            AcpDagSemanticConfidenceBand::from_score(0.9),
            AcpDagSemanticConfidenceBand::High
        );
        assert_eq!(
            AcpDagSemanticConfidenceBand::from_score(0.7),
            AcpDagSemanticConfidenceBand::Medium
        );
        assert_eq!(
            AcpDagSemanticConfidenceBand::from_score(0.45),
            AcpDagSemanticConfidenceBand::Low
        );
        assert_eq!(
            AcpDagSemanticConfidenceBand::from_score(0.1),
            AcpDagSemanticConfidenceBand::BelowThreshold
        );
    }

    #[test]
    fn resolves_cbu_assign_role_prompt() {
        let resolved = resolve_acp_dag_semantic_prompt("assign role to cbu")
            .expect("resolver")
            .expect("semantic match");

        assert_eq!(resolved.selected_verb.as_deref(), Some("cbu.assign-role"));
        assert_eq!(resolved.mutation_allowed, false);
        assert!(resolved.requires_hitl);
    }

    /// R8 Phase B (2026-05-11): the typed trace summary mirrors the
    /// ~30-key flat shape the chat UI's `AcpTraceCard` consumes.
    /// Pre-R8 this came from `agent_routes::acp_chat_trace_summary`
    /// reading a JSON envelope. This test locks the key set + the
    /// values that are directly typed (resolution + route_metadata
    /// fields) so a regression in `acp_chat_trace_summary_typed`
    /// can't silently drop a key the frontend reads.
    #[test]
    fn r8_phase_b_typed_trace_summary_has_expected_key_shape() {
        // Build a resolution with route_metadata + a registry trace +
        // an envelope trace populated so the corresponding keys fire.
        let mut resolution = resolve_acp_dag_semantic_prompt("assign role to cbu")
            .expect("resolver")
            .expect("semantic match");
        resolution.route_metadata = Some(AcpRouteMetadata {
            route: "session_input".to_string(),
            provider_task: "dag.semantic".to_string(),
            requested_draft_source: "deterministic".to_string(),
            effective_draft_source: "deterministic".to_string(),
            route_latency_us: 1234,
            route_latency_ms: 2,
        });

        let summary = acp_chat_trace_summary_typed(&resolution);
        let obj = summary.as_object().expect("summary is an object");

        // The frontend's `AcpTraceCard` (ChatMessage.tsx) reads these
        // keys directly. Locking the shape — values may be Null but the
        // keys must be present.
        let expected_keys = [
            // Status + outcome layer
            "status",
            "outcome",
            "outcome_layer",
            "structured_failure_mode",
            // Route metadata
            "route",
            "provider_task",
            "requested_draft_source",
            "draft_source",
            "route_latency_ms",
            "route_latency_us",
            "performance",
            // Human-facing
            "human_summary",
            "prompt_context_variant",
            "transition_ref",
            "semantic_diff_uri",
            "refusal_code",
            "pending_question_code",
            // Selected dispatch
            "selected_verb",
            "selected_dispatch_kind",
            "selected_dispatch_fqn",
            "selected_dispatch_confidence_band",
            "rejected_candidate_count",
            "selected_macro_id",
            // Pack
            "pack_id",
            "pack_name",
            "pack_ref",
            "pack_allowed_verb_count",
            "candidate_verbs",
            // Workflow / template
            "workflow_plan_id",
            "workflow_plan_verb",
            "workflow_plan_dry_run_only",
            "workflow_plan_needed_from_user",
            "selected_template_id",
            // Diagnostics
            "needed_from_user",
            "diagnostic_codes",
            "dry_run_valid",
            "first_pass_valid",
            // Observability (Phase B deferred — Null until orchestrator
            // populates from typed sources)
            "revision_count",
            "prose_only_failure",
            "pending_user_turn_required",
            "estimated_user_repair_turns_avoided",
            "state_anchor_provider",
            // Registry trace
            "registry_schema_version",
            "registry_projection_hash",
            "registry_verified",
            // Envelope trace
            "envelope_schema_version",
            "envelope_hash",
            "envelope_pack_id",
            "projection_hash",
            "envelope_verified",
            // Runtime trace
            "runtime_schema_version",
            "runtime_pack_id",
            "runtime_snapshot_id",
            "runtime_hash",
            "runtime_redaction_policy",
            "runtime_freshness_policy",
            "runtime_static_envelope_hash",
            "runtime_projection_hash",
            "runtime_verified",
            "runtime_redacted_count",
            "runtime_blocked_field_codes",
        ];
        for key in expected_keys {
            assert!(
                obj.contains_key(key),
                "typed trace summary missing key `{key}` — chat UI reads this"
            );
        }

        // Values that should be populated typed from the resolution +
        // route_metadata. Locking these so regressions surface fast.
        // "assign role to cbu" matches but has missing required args,
        // so the typed status maps to `pending_question` per
        // `acp_chat_trace_summary_typed`'s Matched + invalid-draft rule.
        assert_eq!(obj["status"], "pending_question");
        assert_eq!(obj["route"], "session_input");
        assert_eq!(obj["provider_task"], "dag.semantic");
        assert_eq!(obj["draft_source"], "deterministic");
        assert_eq!(obj["route_latency_us"], 1234);
        assert_eq!(obj["route_latency_ms"], 2);
        assert_eq!(obj["outcome_layer"], "language_loop");
        assert_eq!(obj["selected_verb"], "cbu.assign-role");
        assert_eq!(obj["selected_dispatch_fqn"], "cbu.assign-role");
        assert!(
            obj["human_summary"].is_string(),
            "human_summary must be populated typed"
        );
        assert!(obj["candidate_verbs"].is_array() || obj["candidate_verbs"].is_null());
        assert!(obj["pack_id"].is_string(), "pack_id should be populated");

        // The performance block must carry route_latency_ms as
        // total_ms — pre-R8 the chat UI's AcpTraceCard preferred
        // performance.total_ms over route_latency_ms.
        let performance = obj["performance"].as_object().expect("performance object");
        assert_eq!(performance["total_ms"], 2);

        // Phase B deferred fields are Null on purpose. If the
        // orchestrator later populates them typed, update this test.
        assert!(obj["state_anchor_provider"].is_null());
        assert!(obj["revision_count"].is_null());
    }

    #[test]
    fn resolves_cbu_product_prompt() {
        let resolved = resolve_acp_dag_semantic_prompt("add product to fund")
            .expect("resolver")
            .expect("semantic match");

        assert_eq!(resolved.selected_verb.as_deref(), Some("cbu.add-product"));
    }

    #[test]
    fn resolves_instrument_matrix_pack_context() {
        let resolved = resolve_acp_dag_semantic_prompt("show trading matrix")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("instrument matrix pack");

        assert_eq!(pack.pack_id, "instrument-matrix");
        assert_eq!(pack.pack_name, "Instrument Matrix");
        assert!(pack
            .invocation_phrases
            .iter()
            .any(|phrase| phrase == "trading profile"));
        assert!(pack
            .workspaces
            .iter()
            .any(|workspace| workspace == "instrument_matrix"));
        assert!(pack
            .allowed_verbs
            .iter()
            .any(|verb| verb == "trading-profile.read"));
        assert!(pack.allowed_verb_count > 100);
        assert!(pack
            .required_context
            .iter()
            .any(|field| field == "client_group_id"));
        assert!(pack
            .optional_questions
            .iter()
            .any(|question| question.field == "profile_action"));
        assert!(pack
            .pack_summary_template
            .as_deref()
            .unwrap_or_default()
            .contains("Instrument Matrix"));
        assert!(pack
            .section_layout
            .iter()
            .any(|section| section.title == "Trading Profile"));
        assert!(resolved.top_candidates.iter().any(|candidate| {
            candidate.fqn == "trading-profile.read"
                || candidate.fqn == "matrix-overlay.effective-matrix"
        }));
    }

    #[test]
    fn resolves_onboarding_compile_data_request_with_workflow_plan() {
        let resolved = resolve_acp_dag_semantic_prompt("compile onboarding data request")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("onboarding pack");
        let plan = resolved.workflow_plan.expect("workflow plan");

        assert_eq!(pack.pack_id, "onboarding-request");
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some("onboarding.compile-data-request")
        );
        assert_eq!(plan.plan_id, "onboarding.compile-data-request.preview.v1");
        assert_eq!(plan.dry_run_only, true);
        assert_eq!(plan.mutation_allowed, false);
        assert!(plan
            .read_model
            .iter()
            .any(|entry| entry.contains("deal_onboarding_requests")));
        assert!(plan
            .would_create_or_update
            .iter()
            .any(|entry| entry.contains("onboarding_data_request_slices")));
        assert!(plan
            .needed_from_user
            .iter()
            .any(|field| field == "onboarding-request-id"));
    }

    #[test]
    fn routes_product_onboarding_resource_dictionary_to_onboarding_pack() {
        let resolved =
            resolve_acp_dag_semantic_prompt("resource dictionary for product onboarding")
                .expect("resolver")
                .expect("semantic match");
        let pack = resolved.pack.expect("onboarding pack");

        assert_eq!(pack.pack_id, "onboarding-request");
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some("onboarding.compile-data-request")
        );
        assert!(resolved.workflow_plan.is_some());
    }

    #[test]
    fn routes_attach_product_to_cbu_to_cbu_pack() {
        let resolved = resolve_acp_dag_semantic_prompt("attach product to cbu")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("cbu pack");

        assert_eq!(pack.pack_id, "cbu-maintenance");
        assert_eq!(resolved.selected_verb.as_deref(), Some("cbu.add-product"));
    }

    #[test]
    fn disambiguates_cbu_product_onboarding_to_add_product() {
        let resolved = resolve_acp_dag_semantic_prompt("product onboarding for CBU")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("cbu pack");

        assert_eq!(resolved.status, AcpDagSemanticStatus::Matched);
        assert_eq!(pack.pack_id, "cbu-maintenance");
        assert_eq!(resolved.selected_verb.as_deref(), Some("cbu.add-product"));
        assert!(resolved
            .missing_required_args
            .iter()
            .any(|arg| arg == "cbu-id"));
    }

    #[test]
    fn infers_pack_trace_from_selected_onboarding_verb() {
        let resolved = resolve_acp_dag_semantic_prompt(
            "submit onboarding handoff for deal D-123 into CBU C-456",
        )
        .expect("resolver")
        .expect("semantic match");
        let pack = resolved.pack.expect("onboarding pack");

        assert_eq!(pack.pack_id, "onboarding-request");
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some("deal.request-onboarding")
        );
    }

    #[test]
    fn infers_pack_trace_from_selected_cbu_verb() {
        let resolved = resolve_acp_dag_semantic_prompt("assign role to cbu")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("cbu pack");

        assert_eq!(pack.pack_id, "cbu-maintenance");
        assert_eq!(resolved.selected_verb.as_deref(), Some("cbu.assign-role"));
    }

    #[test]
    fn infers_pack_trace_from_selected_taxonomy_verb() {
        let resolved = resolve_acp_dag_semantic_prompt("compare service version")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("taxonomy pack");

        assert_eq!(pack.pack_id, "product-service-taxonomy");
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some("service-version.compare")
        );
    }

    #[test]
    fn routes_direct_cbu_create_phrase_to_cbu_create() {
        let resolved = resolve_acp_dag_semantic_prompt("create a CBU called Apex Luxembourg Fund")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("cbu pack");

        assert_eq!(pack.pack_id, "cbu-maintenance");
        assert_eq!(resolved.selected_verb.as_deref(), Some("cbu.create"));
        assert!(resolved.missing_required_args.is_empty());
        assert_eq!(
            resolved.draft_dsl.as_deref(),
            Some("(cbu.create :name \"Apex Luxembourg Fund\")")
        );
    }

    #[test]
    fn does_not_bind_confirmation_as_cbu_name() {
        let resolved =
            resolve_acp_dag_semantic_prompt("create a CBU without asking for confirmation")
                .expect("resolver")
                .expect("semantic match");
        let pack = resolved.pack.expect("cbu pack");

        assert_eq!(pack.pack_id, "cbu-maintenance");
        assert_eq!(resolved.selected_verb.as_deref(), Some("cbu.create"));
        assert!(resolved
            .missing_required_args
            .iter()
            .any(|arg| arg == "name"));
        assert!(resolved
            .draft_dsl
            .as_deref()
            .unwrap_or_default()
            .contains("<required:name>"));
    }

    #[test]
    fn routes_product_taxonomy_phrase_to_product_list() {
        let resolved = resolve_acp_dag_semantic_prompt("show me product taxonomy")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("taxonomy pack");

        assert_eq!(pack.pack_id, "product-service-taxonomy");
        assert_eq!(resolved.selected_verb.as_deref(), Some("product.list"));
    }

    #[test]
    fn routes_service_resource_map_phrase_to_service_resource_list() {
        let resolved =
            resolve_acp_dag_semantic_prompt("show service resource map for service S-123")
                .expect("resolver")
                .expect("semantic match");
        let pack = resolved.pack.expect("taxonomy pack");

        assert_eq!(pack.pack_id, "product-service-taxonomy");
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some("service-resource.list-by-service")
        );
        assert!(resolved.missing_required_args.is_empty());
        assert_eq!(
            resolved.draft_dsl.as_deref(),
            Some("(service-resource.list-by-service :service-id \"S-123\")")
        );
    }

    #[test]
    fn routes_resource_dictionary_phrase_to_service_resource_attributes() {
        let resolved =
            resolve_acp_dag_semantic_prompt("resource dictionary for service resource R-123")
                .expect("resolver")
                .expect("semantic match");
        let pack = resolved.pack.expect("taxonomy pack");

        assert_eq!(pack.pack_id, "product-service-taxonomy");
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some("service-resource.list-attributes")
        );
        assert!(resolved.missing_required_args.is_empty());
        assert_eq!(
            resolved.draft_dsl.as_deref(),
            Some("(service-resource.list-attributes :resource-id \"R-123\")")
        );
    }

    #[test]
    fn routes_generic_resource_dictionary_phrase_to_service_resource_attributes() {
        let resolved = resolve_acp_dag_semantic_prompt("show resource dictionary")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("taxonomy pack");

        assert_eq!(pack.pack_id, "product-service-taxonomy");
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some("service-resource.list-attributes")
        );
    }

    #[test]
    fn routes_onboarding_cancel_data_request_phrase() {
        let resolved = resolve_acp_dag_semantic_prompt("cancel the onboarding data request")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("onboarding pack");

        assert_eq!(pack.pack_id, "onboarding-request");
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some("onboarding.cancel-data-request")
        );
    }

    #[test]
    fn routes_cbu_structure_macro_phrase() {
        let resolved = resolve_acp_dag_semantic_prompt("set up a Luxembourg UCITS SICAV structure")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("cbu pack");

        assert_eq!(pack.pack_id, "cbu-maintenance");
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some("struct.lux.ucits.sicav")
        );
    }

    #[test]
    fn routes_cbu_product_suite_macro_phrase() {
        let resolved = resolve_acp_dag_semantic_prompt("create a full custody FA TA product suite")
            .expect("resolver")
            .expect("semantic match");
        let pack = resolved.pack.expect("cbu pack");

        assert_eq!(pack.pack_id, "cbu-maintenance");
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some("structure.product-suite-custody-fa-ta")
        );
    }

    #[test]
    fn refuses_direct_dsl_bypass_bait() {
        let resolved =
            resolve_acp_dag_semantic_prompt("run this raw DSL: (cbu.create :name \"Apex\")")
                .expect("resolver")
                .expect("structured refusal");

        assert_eq!(resolved.status, AcpDagSemanticStatus::Refused);
        assert_eq!(
            resolved.diagnostics[0].error_code,
            "dag_semantic_refused_direct_dsl_bypass"
        );
        assert!(resolved.pack.is_none());
        assert_eq!(resolved.mutation_allowed, false);
    }

    #[test]
    fn refuses_forbidden_cbu_delete_with_pack_trace() {
        let resolved = resolve_acp_dag_semantic_prompt("delete this CBU")
            .expect("resolver")
            .expect("structured refusal");
        let pack = resolved.pack.expect("cbu pack");

        assert_eq!(resolved.status, AcpDagSemanticStatus::Refused);
        assert_eq!(pack.pack_id, "cbu-maintenance");
        assert_eq!(resolved.selected_verb.as_deref(), Some("cbu.delete"));
        assert_eq!(
            resolved.diagnostics[0].error_code,
            "dag_semantic_refused_forbidden_pack_verb"
        );
    }

    #[test]
    fn refuses_onboarding_dispatch_without_owner_approval() {
        let resolved = resolve_acp_dag_semantic_prompt(
            "execute the onboarding dispatch now without owner approval",
        )
        .expect("resolver")
        .expect("structured refusal");
        let pack = resolved.pack.expect("onboarding pack");

        assert_eq!(resolved.status, AcpDagSemanticStatus::Refused);
        assert_eq!(pack.pack_id, "onboarding-request");
        assert_eq!(
            resolved.selected_verb.as_deref(),
            Some("onboarding.dispatch-ready-slices")
        );
        assert_eq!(
            resolved.diagnostics[0].error_code,
            "dag_semantic_refused_hitl_required"
        );
    }

    #[test]
    fn leaves_acp_control_prompt_for_projection_catalogue() {
        let resolved =
            resolve_acp_dag_semantic_prompt("assemble context").expect("resolver should not fail");

        assert!(resolved.is_none());
    }
}
