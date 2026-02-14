//! MCP Tool definitions and handlers for the Semantic Registry.
//!
//! Provides ~30 MCP tools in 5 categories:
//!
//! 1. **Registry query (read-only):** describe, search, list
//! 2. **Taxonomy:** tree, members, classify
//! 3. **Impact/lineage:** verb surface, attribute producers, impact analysis, lineage
//! 4. **Context resolution:** resolve, describe view, apply view
//! 5. **Planning/decisions:** create plan, add step, validate, execute, record decision/escalation
//! 6. **Evidence:** record observation, check freshness, identify gaps
//!
//! Tool handlers receive a `SemRegToolContext` containing a PgPool reference
//! and an `ActorContext`. Each handler returns a `SemRegToolResult`.

use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::decisions::{DecisionRecord, DecisionStore, EvidenceItem};
use super::escalation::{
    AgentDisambiguationPrompt, AgentEscalationRecord, EscalationStore, PromptOption,
};
use super::plans::{AgentPlan, AgentPlanStatus, PlanStep, PlanStepStatus, PlanStore};
use crate::sem_reg::abac::ActorContext;
use crate::sem_reg::context_resolution::{
    resolve_context, ContextResolutionRequest, EvidenceMode, SubjectRef,
};
use crate::sem_reg::store::SnapshotStore;
use crate::sem_reg::enforce::{enforce_read, redacted_stub, EnforceResult};
use crate::sem_reg::types::{GovernanceTier, ObjectType, TrustClass};

// ── Tool Context ──────────────────────────────────────────────

/// Context passed to every MCP tool handler.
pub struct SemRegToolContext<'a> {
    pub pool: &'a PgPool,
    pub actor: &'a ActorContext,
}

// ── Tool Result ───────────────────────────────────────────────

/// Result from an MCP tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemRegToolResult {
    /// Whether the tool succeeded.
    pub success: bool,
    /// Result payload (tool-specific).
    pub data: serde_json::Value,
    /// Error message if failed.
    #[serde(default)]
    pub error: Option<String>,
}

impl SemRegToolResult {
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data,
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: serde_json::Value::Null,
            error: Some(msg.into()),
        }
    }
}

// ── Tool Spec ─────────────────────────────────────────────────

/// Specification for an MCP tool (name, description, parameters).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemRegToolSpec {
    pub name: String,
    pub description: String,
    pub category: String,
    pub parameters: Vec<ToolParameter>,
}

/// A parameter for an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub param_type: String,
    pub required: bool,
}

// ── Tool Registry ─────────────────────────────────────────────

/// Returns all semantic registry MCP tool specifications.
pub fn all_tool_specs() -> Vec<SemRegToolSpec> {
    let mut specs = Vec::new();

    // Category 1: Registry query (read-only)
    specs.extend(registry_query_specs());

    // Category 2: Taxonomy
    specs.extend(taxonomy_specs());

    // Category 3: Impact/lineage
    specs.extend(impact_lineage_specs());

    // Category 4: Context resolution
    specs.extend(context_resolution_specs());

    // Category 5: Planning/decisions
    specs.extend(planning_decision_specs());

    // Category 6: Evidence
    specs.extend(evidence_specs());

    specs
}

// ── Category 1: Registry Query ────────────────────────────────

fn registry_query_specs() -> Vec<SemRegToolSpec> {
    vec![
        SemRegToolSpec {
            name: "sem_reg_describe_attribute".into(),
            description: "Describe an attribute definition by FQN".into(),
            category: "registry_query".into(),
            parameters: vec![param("fqn", "Attribute FQN", "string", true)],
        },
        SemRegToolSpec {
            name: "sem_reg_describe_verb".into(),
            description: "Describe a verb contract by FQN".into(),
            category: "registry_query".into(),
            parameters: vec![param("fqn", "Verb FQN", "string", true)],
        },
        SemRegToolSpec {
            name: "sem_reg_describe_entity_type".into(),
            description: "Describe an entity type definition by FQN".into(),
            category: "registry_query".into(),
            parameters: vec![param("fqn", "Entity type FQN", "string", true)],
        },
        SemRegToolSpec {
            name: "sem_reg_describe_policy".into(),
            description: "Describe a policy rule by FQN".into(),
            category: "registry_query".into(),
            parameters: vec![param("fqn", "Policy FQN", "string", true)],
        },
        SemRegToolSpec {
            name: "sem_reg_search".into(),
            description: "Search registry objects by name/FQN pattern (ILIKE)".into(),
            category: "registry_query".into(),
            parameters: vec![
                param("query", "Search pattern", "string", true),
                param(
                    "object_type",
                    "Filter by object type (optional)",
                    "string",
                    false,
                ),
                param("limit", "Max results (default 20)", "integer", false),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_list_verbs".into(),
            description: "List verb contracts, optionally filtered by domain".into(),
            category: "registry_query".into(),
            parameters: vec![
                param("domain", "Filter by domain (optional)", "string", false),
                param("limit", "Max results (default 50)", "integer", false),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_list_attributes".into(),
            description: "List attribute definitions, optionally filtered by domain".into(),
            category: "registry_query".into(),
            parameters: vec![
                param("domain", "Filter by domain (optional)", "string", false),
                param("limit", "Max results (default 50)", "integer", false),
            ],
        },
    ]
}

// ── Category 2: Taxonomy ──────────────────────────────────────

fn taxonomy_specs() -> Vec<SemRegToolSpec> {
    vec![
        SemRegToolSpec {
            name: "sem_reg_taxonomy_tree".into(),
            description: "Get the taxonomy tree for a taxonomy definition".into(),
            category: "taxonomy".into(),
            parameters: vec![param("taxonomy_fqn", "Taxonomy FQN", "string", true)],
        },
        SemRegToolSpec {
            name: "sem_reg_taxonomy_members".into(),
            description: "List members (objects) classified under a taxonomy node".into(),
            category: "taxonomy".into(),
            parameters: vec![
                param("node_fqn", "Taxonomy node FQN", "string", true),
                param("limit", "Max results (default 50)", "integer", false),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_classify".into(),
            description: "Classify an object under a taxonomy node".into(),
            category: "taxonomy".into(),
            parameters: vec![
                param("object_id", "Object UUID to classify", "uuid", true),
                param("node_fqn", "Target taxonomy node FQN", "string", true),
            ],
        },
    ]
}

// ── Category 3: Impact/Lineage ────────────────────────────────

fn impact_lineage_specs() -> Vec<SemRegToolSpec> {
    vec![
        SemRegToolSpec {
            name: "sem_reg_verb_surface".into(),
            description: "Get the verb surface (inputs/outputs) for a verb contract".into(),
            category: "impact_lineage".into(),
            parameters: vec![param("verb_fqn", "Verb FQN", "string", true)],
        },
        SemRegToolSpec {
            name: "sem_reg_attribute_producers".into(),
            description: "Find verbs that produce (write) a given attribute".into(),
            category: "impact_lineage".into(),
            parameters: vec![param("attribute_fqn", "Attribute FQN", "string", true)],
        },
        SemRegToolSpec {
            name: "sem_reg_impact_analysis".into(),
            description: "Analyse forward or reverse impact for a snapshot via lineage graph"
                .into(),
            category: "impact_lineage".into(),
            parameters: vec![
                param("snapshot_id", "Snapshot UUID to analyse", "uuid", true),
                param(
                    "direction",
                    "forward (what is affected) or reverse (provenance)",
                    "string",
                    false,
                ),
                param(
                    "max_depth",
                    "Max traversal depth (default 5)",
                    "integer",
                    false,
                ),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_lineage".into(),
            description: "Trace lineage forward or reverse for a snapshot".into(),
            category: "impact_lineage".into(),
            parameters: vec![
                param("snapshot_id", "Snapshot UUID", "uuid", true),
                param("direction", "forward or reverse", "string", true),
                param(
                    "max_depth",
                    "Max traversal depth (default 5)",
                    "integer",
                    false,
                ),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_regulation_trace".into(),
            description: "Trace which regulations/policies apply to an object".into(),
            category: "impact_lineage".into(),
            parameters: vec![param("object_fqn", "Object FQN", "string", true)],
        },
    ]
}

// ── Category 4: Context Resolution ────────────────────────────

fn context_resolution_specs() -> Vec<SemRegToolSpec> {
    vec![
        SemRegToolSpec {
            name: "sem_reg_resolve_context".into(),
            description: "Resolve context for a subject — returns ranked verbs, attributes, policies, governance signals".into(),
            category: "context_resolution".into(),
            parameters: vec![
                param("subject_id", "Subject UUID", "uuid", true),
                param("subject_type", "Subject type: case, entity, document, task, view", "string", true),
                param("mode", "Evidence mode: strict, normal, exploratory, governance", "string", false),
                param("intent", "Natural language intent (for embedding ranking)", "string", false),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_describe_view".into(),
            description: "Describe a view definition by FQN".into(),
            category: "context_resolution".into(),
            parameters: vec![param("view_fqn", "View FQN", "string", true)],
        },
        SemRegToolSpec {
            name: "sem_reg_apply_view".into(),
            description: "Apply a view to a subject — returns filtered/ranked attributes and verbs for that view".into(),
            category: "context_resolution".into(),
            parameters: vec![
                param("view_fqn", "View FQN", "string", true),
                param("subject_id", "Subject UUID", "uuid", true),
                param("subject_type", "Subject type", "string", true),
            ],
        },
    ]
}

// ── Category 5: Planning/Decisions ────────────────────────────

fn planning_decision_specs() -> Vec<SemRegToolSpec> {
    vec![
        SemRegToolSpec {
            name: "sem_reg_create_plan".into(),
            description: "Create a new agent plan for a case/subject".into(),
            category: "planning".into(),
            parameters: vec![
                param("goal", "Plan goal description", "string", true),
                param("case_id", "Case UUID (optional)", "uuid", false),
                param("assumptions", "Assumptions (JSON array)", "json", false),
                param("risk_flags", "Risk flags (JSON array)", "json", false),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_add_plan_step".into(),
            description: "Add a step to an existing plan".into(),
            category: "planning".into(),
            parameters: vec![
                param("plan_id", "Plan UUID", "uuid", true),
                param("verb_fqn", "Verb FQN for this step", "string", true),
                param("params", "Step parameters (JSON)", "json", false),
                param(
                    "depends_on",
                    "Step UUIDs this step depends on (JSON array)",
                    "json",
                    false,
                ),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_validate_plan".into(),
            description: "Validate a plan — check preconditions, ABAC, policy compliance".into(),
            category: "planning".into(),
            parameters: vec![param("plan_id", "Plan UUID", "uuid", true)],
        },
        SemRegToolSpec {
            name: "sem_reg_execute_plan_step".into(),
            description: "Execute the next pending step in a plan".into(),
            category: "planning".into(),
            parameters: vec![
                param("plan_id", "Plan UUID", "uuid", true),
                param(
                    "step_id",
                    "Step UUID (optional — executes next pending if omitted)",
                    "uuid",
                    false,
                ),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_record_decision".into(),
            description: "Record a decision with evidence and snapshot manifest".into(),
            category: "planning".into(),
            parameters: vec![
                param("plan_id", "Plan UUID (optional)", "uuid", false),
                param("chosen_action", "Chosen action verb FQN", "string", true),
                param(
                    "description",
                    "Description of what was decided",
                    "string",
                    true,
                ),
                param(
                    "evidence_for",
                    "Supporting evidence (JSON array)",
                    "json",
                    false,
                ),
                param(
                    "evidence_against",
                    "Counter evidence (JSON array)",
                    "json",
                    false,
                ),
                param(
                    "snapshot_manifest",
                    "Object→snapshot ID map (JSON object)",
                    "json",
                    true,
                ),
                param(
                    "confidence",
                    "Decision confidence (0.0-1.0)",
                    "number",
                    true,
                ),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_record_escalation".into(),
            description: "Record an escalation requiring human intervention".into(),
            category: "planning".into(),
            parameters: vec![
                param("decision_id", "Decision UUID (optional)", "uuid", false),
                param("reason", "Reason for escalation", "string", true),
                param(
                    "severity",
                    "Severity: info, warning, critical",
                    "string",
                    true,
                ),
                param(
                    "required_action",
                    "What human action is needed",
                    "string",
                    true,
                ),
                param("assigned_to", "Assign to person/team", "string", false),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_record_disambiguation".into(),
            description: "Record a disambiguation prompt for human clarification".into(),
            category: "planning".into(),
            parameters: vec![
                param("question", "The question to ask", "string", true),
                param(
                    "options",
                    "Available options (JSON array of {id, label, description})",
                    "json",
                    true,
                ),
                param("plan_id", "Plan UUID (optional)", "uuid", false),
            ],
        },
    ]
}

// ── Category 6: Evidence ──────────────────────────────────────

fn evidence_specs() -> Vec<SemRegToolSpec> {
    vec![
        SemRegToolSpec {
            name: "sem_reg_record_observation".into(),
            description: "Record a derivation edge linking input snapshots to an output snapshot"
                .into(),
            category: "evidence".into(),
            parameters: vec![
                param(
                    "verb_fqn",
                    "Verb that produced the derivation",
                    "string",
                    true,
                ),
                param(
                    "input_snapshot_ids",
                    "Array of input snapshot UUIDs",
                    "array",
                    false,
                ),
                param("output_snapshot_id", "Output snapshot UUID", "uuid", true),
                param("run_id", "Optional run record UUID", "uuid", false),
            ],
        },
        SemRegToolSpec {
            name: "sem_reg_check_evidence_freshness".into(),
            description: "Check if embeddings for snapshots are fresh or stale".into(),
            category: "evidence".into(),
            parameters: vec![param(
                "snapshot_ids",
                "Array of snapshot UUIDs to check",
                "array",
                true,
            )],
        },
        SemRegToolSpec {
            name: "sem_reg_identify_evidence_gaps".into(),
            description: "Identify missing or stale evidence for a subject".into(),
            category: "evidence".into(),
            parameters: vec![
                param("subject_id", "Subject UUID", "uuid", true),
                param("subject_type", "Subject type", "string", true),
                param("mode", "Evidence mode: strict, normal", "string", false),
            ],
        },
        // Phase 9: Coverage metrics
        SemRegToolSpec {
            name: "sem_reg_coverage_report".into(),
            description: "Generate governance coverage report across the registry".into(),
            category: "evidence".into(),
            parameters: vec![param(
                "tier",
                "Filter by tier: governed, operational, or all (default)",
                "string",
                false,
            )],
        },
    ]
}

// ── Tool parameter helper ─────────────────────────────────────

fn param(name: &str, description: &str, param_type: &str, required: bool) -> ToolParameter {
    ToolParameter {
        name: name.into(),
        description: description.into(),
        param_type: param_type.into(),
        required,
    }
}

// ── Tool Dispatch ─────────────────────────────────────────────

/// Dispatch an MCP tool call to its handler.
pub async fn dispatch_tool(
    ctx: &SemRegToolContext<'_>,
    tool_name: &str,
    args: &serde_json::Value,
) -> SemRegToolResult {
    match tool_name {
        // Category 1: Registry query
        "sem_reg_describe_attribute" => handle_describe(ctx, args, ObjectType::AttributeDef).await,
        "sem_reg_describe_verb" => handle_describe(ctx, args, ObjectType::VerbContract).await,
        "sem_reg_describe_entity_type" => {
            handle_describe(ctx, args, ObjectType::EntityTypeDef).await
        }
        "sem_reg_describe_policy" => handle_describe(ctx, args, ObjectType::PolicyRule).await,
        "sem_reg_search" => handle_search(ctx, args).await,
        "sem_reg_list_verbs" => handle_list(ctx, args, ObjectType::VerbContract).await,
        "sem_reg_list_attributes" => handle_list(ctx, args, ObjectType::AttributeDef).await,

        // Category 2: Taxonomy
        "sem_reg_taxonomy_tree" => handle_taxonomy_tree(ctx, args).await,
        "sem_reg_taxonomy_members" => handle_taxonomy_members(ctx, args).await,
        "sem_reg_classify" => handle_classify(ctx, args).await,

        // Category 3: Impact/lineage
        "sem_reg_verb_surface" => handle_verb_surface(ctx, args).await,
        "sem_reg_attribute_producers" => handle_attribute_producers(ctx, args).await,
        "sem_reg_impact_analysis" => handle_impact_analysis(ctx, args).await,
        "sem_reg_lineage" => handle_lineage(ctx, args).await,
        "sem_reg_regulation_trace" => handle_regulation_trace(ctx, args).await,

        // Category 4: Context resolution
        "sem_reg_resolve_context" => handle_resolve_context(ctx, args).await,
        "sem_reg_describe_view" => handle_describe(ctx, args, ObjectType::ViewDef).await,
        "sem_reg_apply_view" => handle_apply_view(ctx, args).await,

        // Category 5: Planning/decisions
        "sem_reg_create_plan" => handle_create_plan(ctx, args).await,
        "sem_reg_add_plan_step" => handle_add_plan_step(ctx, args).await,
        "sem_reg_validate_plan" => handle_validate_plan(ctx, args).await,
        "sem_reg_execute_plan_step" => handle_execute_plan_step(ctx, args).await,
        "sem_reg_record_decision" => handle_record_decision(ctx, args).await,
        "sem_reg_record_escalation" => handle_record_escalation(ctx, args).await,
        "sem_reg_record_disambiguation" => handle_record_disambiguation(ctx, args).await,

        // Category 6: Evidence
        "sem_reg_record_observation" => handle_record_observation(ctx, args).await,
        "sem_reg_check_evidence_freshness" => handle_check_freshness(ctx, args).await,
        "sem_reg_identify_evidence_gaps" => handle_identify_gaps(ctx, args).await,
        "sem_reg_coverage_report" => handle_coverage_report(ctx, args).await,

        _ => SemRegToolResult::err(format!("Unknown tool: {}", tool_name)),
    }
}

// ═══════════════════════════════════════════════════════════════
// Tool Handler Implementations
// ═══════════════════════════════════════════════════════════════

// ── Category 1: Registry Query Handlers ───────────────────────

async fn handle_describe(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
    object_type: ObjectType,
) -> SemRegToolResult {
    let fqn = match args.get("fqn").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return SemRegToolResult::err("Missing required parameter: fqn"),
    };

    match SnapshotStore::find_active_by_definition_field(ctx.pool, object_type, "fqn", fqn).await {
        Ok(Some(row)) => {
            // ABAC enforcement
            match enforce_read(ctx.actor, &row) {
                EnforceResult::Deny { reason } => {
                    SemRegToolResult::ok(redacted_stub(&row, &reason))
                }
                _ => {
                    let result = serde_json::json!({
                        "snapshot_id": row.snapshot_id,
                        "object_id": row.object_id,
                        "object_type": row.object_type.to_string(),
                        "version": row.version_string(),
                        "governance_tier": row.governance_tier,
                        "trust_class": row.trust_class,
                        "status": row.status,
                        "definition": row.definition,
                        "created_by": row.created_by,
                        "created_at": row.created_at.to_rfc3339(),
                    });
                    SemRegToolResult::ok(result)
                }
            }
        }
        Ok(None) => SemRegToolResult::err(format!("Not found: {} {}", object_type, fqn)),
        Err(e) => SemRegToolResult::err(format!("Database error: {}", e)),
    }
}

async fn handle_search(ctx: &SemRegToolContext<'_>, args: &serde_json::Value) -> SemRegToolResult {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) => q,
        None => return SemRegToolResult::err("Missing required parameter: query"),
    };
    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(20);

    let object_type_filter = args
        .get("object_type")
        .and_then(|v| v.as_str())
        .and_then(parse_object_type);

    // Search by ILIKE on definition->>'fqn' and definition->>'name'
    let type_clause = match object_type_filter {
        Some(ot) => format!("AND object_type = '{}'", ot),
        None => String::new(),
    };

    let sql = format!(
        r#"
        SELECT snapshot_id, object_id, object_type,
               definition->>'fqn' as fqn, definition->>'name' as name,
               governance_tier, trust_class, created_at
        FROM sem_reg.snapshots
        WHERE status = 'active' AND effective_until IS NULL
          AND (definition->>'fqn' ILIKE $1 OR definition->>'name' ILIKE $1)
          {}
        ORDER BY created_at DESC
        LIMIT $2
        "#,
        type_clause
    );

    let pattern = format!("%{}%", query);
    match sqlx::query_as::<_, SearchResultRow>(&sql)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(ctx.pool)
        .await
    {
        Ok(rows) => {
            let results: Vec<serde_json::Value> = rows
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "snapshot_id": r.snapshot_id,
                        "object_id": r.object_id,
                        "object_type": r.object_type.to_string(),
                        "fqn": r.fqn,
                        "name": r.name,
                        "governance_tier": r.governance_tier,
                        "trust_class": r.trust_class,
                    })
                })
                .collect();
            SemRegToolResult::ok(serde_json::json!({
                "count": results.len(),
                "results": results,
            }))
        }
        Err(e) => SemRegToolResult::err(format!("Search error: {}", e)),
    }
}

async fn handle_list(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
    object_type: ObjectType,
) -> SemRegToolResult {
    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(50);
    let domain = args.get("domain").and_then(|v| v.as_str());

    match SnapshotStore::list_active(ctx.pool, object_type, limit, 0).await {
        Ok(rows) => {
            let mut results: Vec<serde_json::Value> = Vec::new();
            for row in &rows {
                // ABAC enforcement — skip denied rows
                if let EnforceResult::Deny { .. } = enforce_read(ctx.actor, row) {
                    continue;
                }

                let fqn = row
                    .definition
                    .get("fqn")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let name = row
                    .definition
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let obj_domain = row
                    .definition
                    .get("domain")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if let Some(filter_domain) = domain {
                    if obj_domain != filter_domain {
                        continue;
                    }
                }

                results.push(serde_json::json!({
                    "snapshot_id": row.snapshot_id,
                    "object_id": row.object_id,
                    "fqn": fqn,
                    "name": name,
                    "domain": obj_domain,
                    "governance_tier": row.governance_tier,
                    "trust_class": row.trust_class,
                }));
            }
            SemRegToolResult::ok(serde_json::json!({
                "count": results.len(),
                "object_type": object_type.to_string(),
                "results": results,
            }))
        }
        Err(e) => SemRegToolResult::err(format!("List error: {}", e)),
    }
}

// ── Category 2: Taxonomy Handlers ─────────────────────────────

async fn handle_taxonomy_tree(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let fqn = match args.get("taxonomy_fqn").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return SemRegToolResult::err("Missing required parameter: taxonomy_fqn"),
    };

    // Load the taxonomy definition
    match SnapshotStore::find_active_by_definition_field(
        ctx.pool,
        ObjectType::TaxonomyDef,
        "fqn",
        fqn,
    )
    .await
    {
        Ok(Some(row)) => {
            // Load taxonomy nodes
            let nodes = SnapshotStore::list_active(ctx.pool, ObjectType::TaxonomyNode, 500, 0)
                .await
                .unwrap_or_default();

            let tree_nodes: Vec<serde_json::Value> = nodes
                .iter()
                .filter(|n| n.definition.get("taxonomy_fqn").and_then(|v| v.as_str()) == Some(fqn))
                .map(|n| {
                    serde_json::json!({
                        "node_id": n.object_id,
                        "fqn": n.definition.get("fqn").and_then(|v| v.as_str()).unwrap_or(""),
                        "name": n.definition.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        "parent_fqn": n.definition.get("parent_fqn"),
                    })
                })
                .collect();

            SemRegToolResult::ok(serde_json::json!({
                "taxonomy": row.definition,
                "nodes": tree_nodes,
                "node_count": tree_nodes.len(),
            }))
        }
        Ok(None) => SemRegToolResult::err(format!("Taxonomy not found: {}", fqn)),
        Err(e) => SemRegToolResult::err(format!("Database error: {}", e)),
    }
}

async fn handle_taxonomy_members(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let node_fqn = match args.get("node_fqn").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return SemRegToolResult::err("Missing required parameter: node_fqn"),
    };
    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(50);

    // Find membership rules that reference this node
    let memberships =
        SnapshotStore::list_active(ctx.pool, ObjectType::MembershipRule, limit, 0).await;

    match memberships {
        Ok(rows) => {
            let members: Vec<serde_json::Value> = rows
                .iter()
                .filter(|r| {
                    r.definition
                        .get("taxonomy_node_fqn")
                        .and_then(|v| v.as_str())
                        == Some(node_fqn)
                })
                .map(|r| {
                    serde_json::json!({
                        "membership_id": r.object_id,
                        "target_fqn": r.definition.get("target_fqn"),
                        "kind": r.definition.get("kind"),
                    })
                })
                .collect();
            SemRegToolResult::ok(serde_json::json!({
                "node_fqn": node_fqn,
                "member_count": members.len(),
                "members": members,
            }))
        }
        Err(e) => SemRegToolResult::err(format!("Database error: {}", e)),
    }
}

async fn handle_classify(
    _ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    // Classification creates a new MembershipRule snapshot — requires mutation
    // In Phase 8 MVP, this creates a Draft snapshot
    let _object_id = match args
        .get("object_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing or invalid required parameter: object_id"),
    };

    let _node_fqn = match args.get("node_fqn").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return SemRegToolResult::err("Missing required parameter: node_fqn"),
    };

    // MVP: return acknowledgement — full implementation creates a Draft membership rule
    SemRegToolResult::ok(serde_json::json!({
        "status": "acknowledged",
        "message": "Classification request queued. Creates a Draft membership rule.",
    }))
}

// ── Category 3: Impact/Lineage Handlers ───────────────────────

async fn handle_verb_surface(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let verb_fqn = match args.get("verb_fqn").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return SemRegToolResult::err("Missing required parameter: verb_fqn"),
    };

    match SnapshotStore::find_active_by_definition_field(
        ctx.pool,
        ObjectType::VerbContract,
        "fqn",
        verb_fqn,
    )
    .await
    {
        Ok(Some(row)) => {
            let inputs = row.definition.get("inputs").cloned().unwrap_or_default();
            let outputs = row.definition.get("outputs").cloned().unwrap_or_default();
            let preconditions = row
                .definition
                .get("preconditions")
                .cloned()
                .unwrap_or_default();

            SemRegToolResult::ok(serde_json::json!({
                "verb_fqn": verb_fqn,
                "snapshot_id": row.snapshot_id,
                "inputs": inputs,
                "outputs": outputs,
                "preconditions": preconditions,
                "governance_tier": row.governance_tier,
                "trust_class": row.trust_class,
            }))
        }
        Ok(None) => SemRegToolResult::err(format!("Verb not found: {}", verb_fqn)),
        Err(e) => SemRegToolResult::err(format!("Database error: {}", e)),
    }
}

async fn handle_attribute_producers(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let attr_fqn = match args.get("attribute_fqn").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return SemRegToolResult::err("Missing required parameter: attribute_fqn"),
    };

    // Find verb contracts whose outputs reference this attribute
    let verbs = SnapshotStore::list_active(ctx.pool, ObjectType::VerbContract, 500, 0)
        .await
        .unwrap_or_default();

    let producers: Vec<serde_json::Value> = verbs
        .iter()
        .filter(|v| {
            if let Some(outputs) = v.definition.get("outputs").and_then(|o| o.as_array()) {
                outputs.iter().any(|out| {
                    out.get("attribute_fqn")
                        .and_then(|f| f.as_str())
                        .map(|f| f == attr_fqn)
                        .unwrap_or(false)
                })
            } else {
                false
            }
        })
        .map(|v| {
            serde_json::json!({
                "verb_fqn": v.definition.get("fqn"),
                "snapshot_id": v.snapshot_id,
            })
        })
        .collect();

    SemRegToolResult::ok(serde_json::json!({
        "attribute_fqn": attr_fqn,
        "producer_count": producers.len(),
        "producers": producers,
    }))
}

async fn handle_impact_analysis(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let snapshot_id = match args
        .get("snapshot_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing or invalid required parameter: snapshot_id"),
    };

    let direction = args
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("forward");

    let max_depth = args.get("max_depth").and_then(|v| v.as_i64()).unwrap_or(5) as i32;

    use crate::sem_reg::projections::lineage::LineageStore;

    let result = match direction {
        "forward" => LineageStore::query_forward_impact(ctx.pool, snapshot_id, max_depth).await,
        "reverse" => LineageStore::query_reverse_provenance(ctx.pool, snapshot_id, max_depth).await,
        _ => {
            return SemRegToolResult::err(format!(
                "Invalid direction: '{}'. Use 'forward' or 'reverse'.",
                direction
            ))
        }
    };

    match result {
        Ok(nodes) => {
            let items: Vec<serde_json::Value> = nodes
                .iter()
                .map(|n| {
                    serde_json::json!({
                        "snapshot_id": n.snapshot_id,
                        "object_type": n.object_type,
                        "object_id": n.object_id,
                        "depth": n.depth,
                        "via_verb": n.via_verb,
                        "via_edge_id": n.via_edge_id,
                    })
                })
                .collect();
            SemRegToolResult::ok(serde_json::json!({
                "snapshot_id": snapshot_id,
                "direction": direction,
                "max_depth": max_depth,
                "count": items.len(),
                "nodes": items,
            }))
        }
        Err(e) => SemRegToolResult::err(format!("Impact analysis query failed: {}", e)),
    }
}

async fn handle_lineage(ctx: &SemRegToolContext<'_>, args: &serde_json::Value) -> SemRegToolResult {
    let snapshot_id = match args
        .get("snapshot_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing or invalid required parameter: snapshot_id"),
    };

    let direction = args
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("reverse");

    let max_depth = args.get("max_depth").and_then(|v| v.as_i64()).unwrap_or(10) as i32;

    use crate::sem_reg::projections::lineage::LineageStore;

    let nodes = match direction {
        "forward" => LineageStore::query_forward_impact(ctx.pool, snapshot_id, max_depth).await,
        _ => LineageStore::query_reverse_provenance(ctx.pool, snapshot_id, max_depth).await,
    };

    match nodes {
        Ok(nodes) => {
            let edges: Vec<serde_json::Value> = nodes
                .iter()
                .map(|n| {
                    serde_json::json!({
                        "snapshot_id": n.snapshot_id,
                        "object_type": n.object_type,
                        "object_id": n.object_id,
                        "depth": n.depth,
                        "via_verb": n.via_verb,
                        "via_edge_id": n.via_edge_id,
                    })
                })
                .collect();
            SemRegToolResult::ok(serde_json::json!({
                "snapshot_id": snapshot_id,
                "direction": direction,
                "edge_count": edges.len(),
                "edges": edges,
            }))
        }
        Err(e) => SemRegToolResult::err(format!("Lineage query failed: {}", e)),
    }
}

async fn handle_regulation_trace(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let object_fqn = match args.get("object_fqn").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return SemRegToolResult::err("Missing required parameter: object_fqn"),
    };

    // Find policies that reference this object
    let policies = SnapshotStore::list_active(ctx.pool, ObjectType::PolicyRule, 200, 0)
        .await
        .unwrap_or_default();

    let applicable: Vec<serde_json::Value> = policies
        .iter()
        .filter(|p| {
            p.definition
                .get("predicates")
                .and_then(|preds| preds.as_array())
                .map(|preds| {
                    preds.iter().any(|pred| {
                        pred.get("field")
                            .and_then(|f| f.as_str())
                            .map(|f| f.contains(object_fqn) || object_fqn.contains(f))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .map(|p| {
            serde_json::json!({
                "policy_fqn": p.definition.get("fqn"),
                "policy_name": p.definition.get("name"),
                "snapshot_id": p.snapshot_id,
            })
        })
        .collect();

    SemRegToolResult::ok(serde_json::json!({
        "object_fqn": object_fqn,
        "applicable_policies": applicable,
        "policy_count": applicable.len(),
    }))
}

// ── Category 4: Context Resolution Handlers ───────────────────

async fn handle_resolve_context(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let subject_id = match args
        .get("subject_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing or invalid required parameter: subject_id"),
    };

    let subject_type = args
        .get("subject_type")
        .and_then(|v| v.as_str())
        .unwrap_or("entity");

    let subject = match subject_type {
        "case" => SubjectRef::CaseId(subject_id),
        "entity" => SubjectRef::EntityId(subject_id),
        "document" => SubjectRef::DocumentId(subject_id),
        "task" => SubjectRef::TaskId(subject_id),
        "view" => SubjectRef::ViewId(subject_id),
        _ => return SemRegToolResult::err(format!("Invalid subject_type: {}", subject_type)),
    };

    let mode = args
        .get("mode")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "strict" => EvidenceMode::Strict,
            "exploratory" => EvidenceMode::Exploratory,
            "governance" => EvidenceMode::Governance,
            _ => EvidenceMode::Normal,
        })
        .unwrap_or(EvidenceMode::Normal);

    let intent = args
        .get("intent")
        .and_then(|v| v.as_str())
        .map(String::from);

    let request = ContextResolutionRequest {
        subject,
        intent,
        actor: ctx.actor.clone(),
        goals: vec![],
        constraints: Default::default(),
        evidence_mode: mode,
        point_in_time: None,
    };

    match resolve_context(ctx.pool, &request).await {
        Ok(response) => match serde_json::to_value(&response) {
            Ok(json) => SemRegToolResult::ok(json),
            Err(e) => SemRegToolResult::err(format!("Serialization error: {}", e)),
        },
        Err(e) => SemRegToolResult::err(format!("Resolution error: {}", e)),
    }
}

async fn handle_apply_view(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let view_fqn = match args.get("view_fqn").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return SemRegToolResult::err("Missing required parameter: view_fqn"),
    };

    let subject_id = match args
        .get("subject_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing or invalid required parameter: subject_id"),
    };

    let subject_type = args
        .get("subject_type")
        .and_then(|v| v.as_str())
        .unwrap_or("entity");

    // Load the view definition
    let view_row = match SnapshotStore::find_active_by_definition_field(
        ctx.pool,
        ObjectType::ViewDef,
        "fqn",
        view_fqn,
    )
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return SemRegToolResult::err(format!("View not found: {}", view_fqn)),
        Err(e) => return SemRegToolResult::err(format!("Database error: {}", e)),
    };

    // Resolve context with the view
    let subject = match subject_type {
        "case" => SubjectRef::CaseId(subject_id),
        "entity" => SubjectRef::EntityId(subject_id),
        "document" => SubjectRef::DocumentId(subject_id),
        "task" => SubjectRef::TaskId(subject_id),
        "view" => SubjectRef::ViewId(subject_id),
        _ => return SemRegToolResult::err(format!("Invalid subject_type: {}", subject_type)),
    };

    let request = ContextResolutionRequest {
        subject,
        intent: None,
        actor: ctx.actor.clone(),
        goals: vec![],
        constraints: Default::default(),
        evidence_mode: EvidenceMode::Normal,
        point_in_time: None,
    };

    match resolve_context(ctx.pool, &request).await {
        Ok(response) => SemRegToolResult::ok(serde_json::json!({
            "view_fqn": view_fqn,
            "view_snapshot_id": view_row.snapshot_id,
            "candidate_verbs": response.candidate_verbs.len(),
            "candidate_attributes": response.candidate_attributes.len(),
            "policy_verdicts": response.policy_verdicts.len(),
            "confidence": response.confidence,
        })),
        Err(e) => SemRegToolResult::err(format!("Resolution error: {}", e)),
    }
}

// ── Category 5: Planning/Decision Handlers ────────────────────

async fn handle_create_plan(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let goal = match args.get("goal").and_then(|v| v.as_str()) {
        Some(g) => g.to_string(),
        None => return SemRegToolResult::err("Missing required parameter: goal"),
    };

    let case_id = args
        .get("case_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    let assumptions: Vec<String> = args
        .get("assumptions")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let risk_flags: Vec<String> = args
        .get("risk_flags")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let plan = AgentPlan {
        plan_id: Uuid::new_v4(),
        case_id,
        goal,
        context_resolution_ref: None,
        steps: vec![],
        assumptions,
        risk_flags,
        security_clearance: None,
        status: AgentPlanStatus::Draft,
        created_by: ctx.actor.actor_id.clone(),
        created_at: Utc::now(),
        updated_at: None,
    };

    match PlanStore::insert_plan(ctx.pool, &plan).await {
        Ok(plan_id) => SemRegToolResult::ok(serde_json::json!({
            "plan_id": plan_id,
            "status": "draft",
        })),
        Err(e) => SemRegToolResult::err(format!("Failed to create plan: {}", e)),
    }
}

async fn handle_add_plan_step(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let plan_id = match args
        .get("plan_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing or invalid required parameter: plan_id"),
    };

    let verb_fqn = match args.get("verb_fqn").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return SemRegToolResult::err("Missing required parameter: verb_fqn"),
    };

    // Resolve verb FQN to get its snapshot ID
    let verb_row = match SnapshotStore::find_active_by_definition_field(
        ctx.pool,
        ObjectType::VerbContract,
        "fqn",
        verb_fqn,
    )
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return SemRegToolResult::err(format!("Verb not found: {}", verb_fqn)),
        Err(e) => return SemRegToolResult::err(format!("Database error: {}", e)),
    };

    // Determine step sequence
    let existing_steps = PlanStore::load_steps(ctx.pool, plan_id)
        .await
        .unwrap_or_default();
    let next_seq = existing_steps.len() as i32;

    let params = args.get("params").cloned().unwrap_or(serde_json::json!({}));

    let depends_on: Vec<Uuid> = args
        .get("depends_on")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let step = PlanStep {
        step_id: Uuid::new_v4(),
        plan_id,
        seq: next_seq,
        verb_id: verb_row.object_id,
        verb_snapshot_id: verb_row.snapshot_id,
        verb_fqn: verb_fqn.to_string(),
        params,
        expected_postconditions: vec![],
        fallback_steps: vec![],
        depends_on_steps: depends_on,
        status: PlanStepStatus::Pending,
        result: None,
        error: None,
    };

    match PlanStore::insert_step(ctx.pool, &step).await {
        Ok(step_id) => SemRegToolResult::ok(serde_json::json!({
            "step_id": step_id,
            "plan_id": plan_id,
            "seq": next_seq,
            "verb_fqn": verb_fqn,
            "verb_snapshot_id": verb_row.snapshot_id,
        })),
        Err(e) => SemRegToolResult::err(format!("Failed to add step: {}", e)),
    }
}

async fn handle_validate_plan(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let plan_id = match args
        .get("plan_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing or invalid required parameter: plan_id"),
    };

    let plan = match PlanStore::load_plan(ctx.pool, plan_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return SemRegToolResult::err(format!("Plan not found: {}", plan_id)),
        Err(e) => return SemRegToolResult::err(format!("Database error: {}", e)),
    };

    let steps = PlanStore::load_steps(ctx.pool, plan_id)
        .await
        .unwrap_or_default();

    let mut issues: Vec<serde_json::Value> = Vec::new();

    // Check: plan has at least one step
    if steps.is_empty() {
        issues.push(serde_json::json!({
            "severity": "error",
            "message": "Plan has no steps",
        }));
    }

    // Check: dependency references are valid
    let step_ids: Vec<Uuid> = steps.iter().map(|s| s.step_id).collect();
    for step in &steps {
        for dep in &step.depends_on_steps {
            if !step_ids.contains(dep) {
                issues.push(serde_json::json!({
                    "severity": "error",
                    "message": format!("Step {} depends on non-existent step {}", step.step_id, dep),
                }));
            }
        }
    }

    // Check: no circular dependencies (simple check — flag if step depends on later step)
    for step in &steps {
        for dep in &step.depends_on_steps {
            if let Some(dep_step) = steps.iter().find(|s| s.step_id == *dep) {
                if dep_step.seq >= step.seq {
                    issues.push(serde_json::json!({
                        "severity": "warning",
                        "message": format!("Step {} (seq {}) depends on step {} (seq {}) — possible ordering issue",
                            step.step_id, step.seq, dep, dep_step.seq),
                    }));
                }
            }
        }
    }

    let valid = issues
        .iter()
        .all(|i| i.get("severity").and_then(|s| s.as_str()) != Some("error"));

    SemRegToolResult::ok(serde_json::json!({
        "plan_id": plan_id,
        "goal": plan.goal,
        "step_count": steps.len(),
        "valid": valid,
        "issues": issues,
    }))
}

async fn handle_execute_plan_step(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let plan_id = match args
        .get("plan_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing or invalid required parameter: plan_id"),
    };

    let step_id = args
        .get("step_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    // Load steps
    let steps = PlanStore::load_steps(ctx.pool, plan_id)
        .await
        .unwrap_or_default();

    // Find the step to execute
    let target_step = if let Some(sid) = step_id {
        steps.iter().find(|s| s.step_id == sid)
    } else {
        // Find next pending step
        steps.iter().find(|s| s.status == PlanStepStatus::Pending)
    };

    let target = match target_step {
        Some(s) => s,
        None => return SemRegToolResult::err("No pending step found to execute"),
    };

    // Check dependencies
    for dep_id in &target.depends_on_steps {
        if let Some(dep) = steps.iter().find(|s| s.step_id == *dep_id) {
            if dep.status != PlanStepStatus::Completed {
                return SemRegToolResult::err(format!(
                    "Dependency {} (verb: {}) is not completed (status: {})",
                    dep.step_id, dep.verb_fqn, dep.status
                ));
            }
        }
    }

    // Mark step as running
    let _ = PlanStore::update_step_status(
        ctx.pool,
        target.step_id,
        PlanStepStatus::Running,
        None,
        None,
    )
    .await;

    // Activate plan if still in draft
    let _ = PlanStore::update_plan_status(ctx.pool, plan_id, AgentPlanStatus::Active).await;

    // MVP: mark as completed (actual verb execution would be wired through DSL executor)
    let result_value = serde_json::json!({
        "status": "executed",
        "verb_fqn": target.verb_fqn,
        "verb_snapshot_id": target.verb_snapshot_id,
        "message": "Step execution recorded. Wire DSL executor for actual verb execution.",
    });

    let _ = PlanStore::update_step_status(
        ctx.pool,
        target.step_id,
        PlanStepStatus::Completed,
        Some(&result_value),
        None,
    )
    .await;

    SemRegToolResult::ok(serde_json::json!({
        "step_id": target.step_id,
        "plan_id": plan_id,
        "verb_fqn": target.verb_fqn,
        "status": "completed",
        "result": result_value,
    }))
}

async fn handle_record_decision(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let chosen_action = match args.get("chosen_action").and_then(|v| v.as_str()) {
        Some(a) => a.to_string(),
        None => return SemRegToolResult::err("Missing required parameter: chosen_action"),
    };

    let description = match args.get("description").and_then(|v| v.as_str()) {
        Some(d) => d.to_string(),
        None => return SemRegToolResult::err("Missing required parameter: description"),
    };

    let manifest: HashMap<Uuid, Uuid> = args
        .get("snapshot_manifest")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let confidence = args
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);

    let plan_id = args
        .get("plan_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    let evidence_for: Vec<EvidenceItem> = args
        .get("evidence_for")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let evidence_against: Vec<EvidenceItem> = args
        .get("evidence_against")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let escalation_flag = confidence < 0.3;

    let record = DecisionRecord {
        decision_id: Uuid::new_v4(),
        plan_id,
        step_id: None,
        context_ref: None,
        chosen_action,
        chosen_action_description: description,
        alternatives_considered: vec![],
        evidence_for,
        evidence_against,
        negative_evidence: vec![],
        policy_verdicts: vec![],
        snapshot_manifest: manifest,
        confidence,
        escalation_flag,
        escalation_id: None,
        decided_by: ctx.actor.actor_id.clone(),
        decided_at: Utc::now(),
    };

    match DecisionStore::insert(ctx.pool, &record).await {
        Ok(decision_id) => SemRegToolResult::ok(serde_json::json!({
            "decision_id": decision_id,
            "escalation_flag": escalation_flag,
            "confidence": confidence,
        })),
        Err(e) => SemRegToolResult::err(format!("Failed to record decision: {}", e)),
    }
}

async fn handle_record_escalation(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let reason = match args.get("reason").and_then(|v| v.as_str()) {
        Some(r) => r.to_string(),
        None => return SemRegToolResult::err("Missing required parameter: reason"),
    };

    let severity = args
        .get("severity")
        .and_then(|v| v.as_str())
        .unwrap_or("warning")
        .to_string();

    let required_action = match args.get("required_action").and_then(|v| v.as_str()) {
        Some(a) => a.to_string(),
        None => return SemRegToolResult::err("Missing required parameter: required_action"),
    };

    let decision_id = args
        .get("decision_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    let assigned_to = args
        .get("assigned_to")
        .and_then(|v| v.as_str())
        .map(String::from);

    let record = AgentEscalationRecord {
        escalation_id: Uuid::new_v4(),
        decision_id,
        reason,
        severity,
        context_snapshot: None,
        required_human_action: required_action,
        assigned_to,
        resolved_at: None,
        resolution: None,
        created_by: ctx.actor.actor_id.clone(),
        created_at: Utc::now(),
    };

    match EscalationStore::insert_escalation(ctx.pool, &record).await {
        Ok(escalation_id) => SemRegToolResult::ok(serde_json::json!({
            "escalation_id": escalation_id,
            "severity": record.severity,
        })),
        Err(e) => SemRegToolResult::err(format!("Failed to record escalation: {}", e)),
    }
}

async fn handle_record_disambiguation(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let question = match args.get("question").and_then(|v| v.as_str()) {
        Some(q) => q.to_string(),
        None => return SemRegToolResult::err("Missing required parameter: question"),
    };

    let options: Vec<PromptOption> = match args.get("options") {
        Some(v) => match serde_json::from_value(v.clone()) {
            Ok(opts) => opts,
            Err(e) => return SemRegToolResult::err(format!("Invalid options format: {}", e)),
        },
        None => return SemRegToolResult::err("Missing required parameter: options"),
    };

    let plan_id = args
        .get("plan_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    let prompt = AgentDisambiguationPrompt {
        prompt_id: Uuid::new_v4(),
        decision_id: None,
        plan_id,
        question,
        options,
        context_snapshot: None,
        answered: false,
        chosen_option: None,
        answered_by: None,
        answered_at: None,
        created_at: Utc::now(),
    };

    match EscalationStore::insert_prompt(ctx.pool, &prompt).await {
        Ok(prompt_id) => SemRegToolResult::ok(serde_json::json!({
            "prompt_id": prompt_id,
            "status": "pending",
        })),
        Err(e) => SemRegToolResult::err(format!("Failed to record disambiguation: {}", e)),
    }
}

// ── Category 6: Evidence Handlers ─────────────────────────────

async fn handle_record_observation(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    use crate::sem_reg::projections::lineage::LineageStore;

    let verb_fqn = match args.get("verb_fqn").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return SemRegToolResult::err("Missing required parameter: verb_fqn"),
    };

    let input_snapshot_ids: Vec<Uuid> = args
        .get("input_snapshot_ids")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                .collect()
        })
        .unwrap_or_default();

    let output_snapshot_id = match args
        .get("output_snapshot_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => {
            return SemRegToolResult::err(
                "Missing or invalid required parameter: output_snapshot_id",
            )
        }
    };

    let run_id = args
        .get("run_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    match LineageStore::record_derivation_edge(
        ctx.pool,
        &input_snapshot_ids,
        output_snapshot_id,
        verb_fqn,
        run_id,
    )
    .await
    {
        Ok(edge_id) => SemRegToolResult::ok(serde_json::json!({
            "edge_id": edge_id,
            "verb_fqn": verb_fqn,
            "status": "recorded",
        })),
        Err(e) => SemRegToolResult::err(format!("Failed to record observation: {}", e)),
    }
}

async fn handle_check_freshness(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let snapshot_ids: Vec<Uuid> = args
        .get("snapshot_ids")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                .collect()
        })
        .unwrap_or_default();

    if snapshot_ids.is_empty() {
        return SemRegToolResult::err("Missing required parameter: snapshot_ids (array of UUIDs)");
    }

    use crate::sem_reg::projections::embeddings::EmbeddingStore;

    let mut stale_items = Vec::new();

    for sid in &snapshot_ids {
        // Compute current hash from the snapshot definition
        let current_hash: Option<String> = sqlx::query_scalar(
            "SELECT md5(definition::text) FROM sem_reg.snapshots WHERE snapshot_id = $1",
        )
        .bind(sid)
        .fetch_optional(ctx.pool)
        .await
        .unwrap_or(None);

        let Some(hash) = current_hash else {
            stale_items.push(serde_json::json!({
                "snapshot_id": sid,
                "stale": true,
                "reason": "snapshot_not_found",
            }));
            continue;
        };

        match EmbeddingStore::check_staleness(ctx.pool, *sid, &hash).await {
            Ok(is_stale) => {
                if is_stale {
                    stale_items.push(serde_json::json!({
                        "snapshot_id": sid,
                        "stale": true,
                    }));
                }
            }
            Err(_) => {
                stale_items.push(serde_json::json!({
                    "snapshot_id": sid,
                    "stale": true,
                    "reason": "no_embedding_record",
                }));
            }
        }
    }

    SemRegToolResult::ok(serde_json::json!({
        "checked_count": snapshot_ids.len(),
        "all_fresh": stale_items.is_empty(),
        "stale_count": stale_items.len(),
        "stale_items": stale_items,
    }))
}

async fn handle_identify_gaps(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let subject_id = match args
        .get("subject_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing or invalid required parameter: subject_id"),
    };

    let subject_type = args
        .get("subject_type")
        .and_then(|v| v.as_str())
        .unwrap_or("entity");

    let mode = args
        .get("mode")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "strict" => EvidenceMode::Strict,
            _ => EvidenceMode::Normal,
        })
        .unwrap_or(EvidenceMode::Normal);

    // Use context resolution to identify gaps
    let subject = match subject_type {
        "case" => SubjectRef::CaseId(subject_id),
        "entity" => SubjectRef::EntityId(subject_id),
        _ => SubjectRef::EntityId(subject_id),
    };

    let request = ContextResolutionRequest {
        subject,
        intent: None,
        actor: ctx.actor.clone(),
        goals: vec!["identify_evidence_gaps".into()],
        constraints: Default::default(),
        evidence_mode: mode,
        point_in_time: None,
    };

    match resolve_context(ctx.pool, &request).await {
        Ok(response) => {
            let gaps: Vec<serde_json::Value> = response
                .candidate_attributes
                .iter()
                .filter(|a| a.required && !a.present)
                .map(|a| {
                    serde_json::json!({
                        "attribute_fqn": a.fqn,
                        "attribute_name": a.name,
                        "governance_tier": a.governance_tier,
                        "trust_class": a.trust_class,
                    })
                })
                .collect();

            SemRegToolResult::ok(serde_json::json!({
                "subject_id": subject_id,
                "gap_count": gaps.len(),
                "gaps": gaps,
                "governance_signals": response.governance_signals.len(),
            }))
        }
        Err(e) => SemRegToolResult::err(format!("Gap identification error: {}", e)),
    }
}

async fn handle_coverage_report(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let tier_filter = args
        .get("tier")
        .and_then(|v| v.as_str())
        .and_then(|s| match s {
            "governed" | "operational" => Some(s.to_string()),
            _ => None,
        });

    match crate::sem_reg::MetricsStore::coverage_report(ctx.pool, tier_filter.as_deref()).await {
        Ok(report) => SemRegToolResult::ok(serde_json::to_value(&report).unwrap_or_default()),
        Err(e) => SemRegToolResult::err(format!("Coverage report failed: {}", e)),
    }
}

// ── Helper types ──────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct SearchResultRow {
    snapshot_id: Uuid,
    object_id: Uuid,
    object_type: ObjectType,
    fqn: Option<String>,
    name: Option<String>,
    governance_tier: GovernanceTier,
    trust_class: TrustClass,
}

fn parse_object_type(s: &str) -> Option<ObjectType> {
    match s {
        "attribute_def" => Some(ObjectType::AttributeDef),
        "entity_type_def" => Some(ObjectType::EntityTypeDef),
        "verb_contract" => Some(ObjectType::VerbContract),
        "taxonomy_def" => Some(ObjectType::TaxonomyDef),
        "taxonomy_node" => Some(ObjectType::TaxonomyNode),
        "membership_rule" => Some(ObjectType::MembershipRule),
        "view_def" => Some(ObjectType::ViewDef),
        "policy_rule" => Some(ObjectType::PolicyRule),
        "evidence_requirement" => Some(ObjectType::EvidenceRequirement),
        "document_type_def" => Some(ObjectType::DocumentTypeDef),
        "observation_def" => Some(ObjectType::ObservationDef),
        "derivation_spec" => Some(ObjectType::DerivationSpec),
        _ => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tool_specs_count() {
        let specs = all_tool_specs();
        assert!(
            specs.len() >= 28,
            "Expected at least 28 tools, got {}",
            specs.len()
        );
    }

    #[test]
    fn test_tool_spec_categories() {
        let specs = all_tool_specs();
        let categories: Vec<String> = specs.iter().map(|s| s.category.clone()).collect();
        assert!(categories.contains(&"registry_query".to_string()));
        assert!(categories.contains(&"taxonomy".to_string()));
        assert!(categories.contains(&"impact_lineage".to_string()));
        assert!(categories.contains(&"context_resolution".to_string()));
        assert!(categories.contains(&"planning".to_string()));
        assert!(categories.contains(&"evidence".to_string()));
    }

    #[test]
    fn test_tool_names_unique() {
        let specs = all_tool_specs();
        let mut names: Vec<String> = specs.iter().map(|s| s.name.clone()).collect();
        let len_before = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), len_before, "Duplicate tool names found");
    }

    #[test]
    fn test_tool_result_ok() {
        let result = SemRegToolResult::ok(serde_json::json!({"key": "value"}));
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_tool_result_err() {
        let result = SemRegToolResult::err("something went wrong");
        assert!(!result.success);
        assert_eq!(result.error, Some("something went wrong".into()));
    }

    #[test]
    fn test_parse_object_type() {
        assert_eq!(
            parse_object_type("verb_contract"),
            Some(ObjectType::VerbContract)
        );
        assert_eq!(
            parse_object_type("attribute_def"),
            Some(ObjectType::AttributeDef)
        );
        assert_eq!(parse_object_type("invalid"), None);
    }

    #[test]
    fn test_tool_specs_have_required_params() {
        let specs = all_tool_specs();
        for spec in &specs {
            // Every tool should have a description
            assert!(
                !spec.description.is_empty(),
                "Tool {} missing description",
                spec.name
            );
            // Verify required params have types
            for param in &spec.parameters {
                assert!(
                    !param.param_type.is_empty(),
                    "Tool {} param {} missing type",
                    spec.name,
                    param.name
                );
            }
        }
    }
}
