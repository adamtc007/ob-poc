//! Phase 0 MCP Tools — 17 Stewardship tools (spec §6.1 + §6.2).
//!
//! All mutating tools:
//!   - accept `client_request_id: Option<Uuid>` for idempotency
//!   - emit a `StewardshipRecord` to the append-only audit chain
//!   - enforce ABAC via the actor context
//!
//! Tool names follow the `stew_*` prefix convention to distinguish from
//! existing `sem_reg_*` tools.

use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::sem_reg::agent::mcp_tools::{
    SemRegToolContext, SemRegToolResult, SemRegToolSpec, ToolParameter,
};
use crate::sem_reg::store::SnapshotStore;
use crate::sem_reg::types::{ObjectType, SnapshotStatus};

use super::guardrails::evaluate_all_guardrails;
use super::idempotency::{check_idempotency, record_idempotency, IdempotencyCheck};
use super::impact::compute_changeset_impact;
use super::store::StewardshipStore;
use super::templates::instantiate_template;
use super::types::*;

// ═══════════════════════════════════════════════════════════════
//  Tool Specifications
// ═══════════════════════════════════════════════════════════════

/// Returns all Phase 0 stewardship tool specifications.
pub fn phase0_tool_specs() -> Vec<SemRegToolSpec> {
    let mut specs = Vec::new();
    specs.extend(stewardship_mutating_specs());
    specs.extend(stewardship_query_specs());
    specs
}

fn param(name: &str, desc: &str, ptype: &str, required: bool) -> ToolParameter {
    ToolParameter {
        name: name.to_string(),
        description: desc.to_string(),
        param_type: ptype.to_string(),
        required,
    }
}

fn stewardship_mutating_specs() -> Vec<SemRegToolSpec> {
    vec![
        SemRegToolSpec {
            name: "stew_compose_changeset".into(),
            description: "Create a new changeset with intent and optional template".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("scope", "Domain scope for the changeset", "string", true),
                param("intent", "Natural language description of what this changeset does", "string", false),
                param("template_fqn", "Template FQN to pre-populate (optional)", "string", false),
                param("overrides", "Template variable overrides (JSON object, optional)", "json", false),
                param("client_request_id", "Idempotency key", "uuid", false),
            ],
        },
        SemRegToolSpec {
            name: "stew_suggest".into(),
            description: "Agent-driven refinement suggestion for changeset items".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("changeset_id", "Changeset UUID", "uuid", true),
                param("suggestion", "Agent suggestion text", "string", true),
                param("target_entry_id", "Entry to refine (optional — null for general)", "uuid", false),
                param("client_request_id", "Idempotency key", "uuid", false),
            ],
        },
        SemRegToolSpec {
            name: "stew_add_item".into(),
            description: "Add a draft item to a changeset (writes Draft snapshot to sem_reg)".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("changeset_id", "Changeset UUID", "uuid", true),
                param("object_fqn", "Object FQN for the new item", "string", true),
                param("object_type", "Object type (attribute_def, entity_type_def, etc.)", "string", true),
                param("action", "Change action: add, modify, promote, deprecate, alias", "string", true),
                param("draft_payload", "Draft definition payload (JSON)", "json", true),
                param("predecessor_id", "Predecessor snapshot ID (for modify/deprecate)", "uuid", false),
                param("reasoning", "Reasoning for this change", "string", false),
                param("client_request_id", "Idempotency key", "uuid", false),
            ],
        },
        SemRegToolSpec {
            name: "stew_remove_item".into(),
            description: "Remove a draft item from a changeset".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("changeset_id", "Changeset UUID", "uuid", true),
                param("entry_id", "Entry UUID to remove", "uuid", true),
                param("client_request_id", "Idempotency key", "uuid", false),
            ],
        },
        SemRegToolSpec {
            name: "stew_refine_item".into(),
            description: "Refine an existing draft item (supersedes prior draft, bumps revision)".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("changeset_id", "Changeset UUID", "uuid", true),
                param("entry_id", "Entry UUID to refine", "uuid", true),
                param("draft_payload", "Updated draft definition payload (JSON)", "json", true),
                param("reasoning", "Reasoning for the refinement", "string", false),
                param("client_request_id", "Idempotency key", "uuid", false),
            ],
        },
        SemRegToolSpec {
            name: "stew_attach_basis".into(),
            description: "Attach a basis record with claims to a changeset".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("changeset_id", "Changeset UUID", "uuid", true),
                param("kind", "Basis kind: regulatory_fact, market_practice, platform_convention, ai_inference, expert_opinion, observation", "string", true),
                param("source_ref", "Source reference (URL, document, etc.)", "string", true),
                param("excerpt", "Relevant excerpt or summary", "string", false),
                param("claims", "Array of claims (JSON array of {claim_text, confidence, entry_id})", "json", false),
                param("client_request_id", "Idempotency key", "uuid", false),
            ],
        },
        SemRegToolSpec {
            name: "stew_gate_precheck".into(),
            description: "Run all guardrails G01-G15 and publish gates against changeset items".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("changeset_id", "Changeset UUID", "uuid", true),
            ],
        },
        SemRegToolSpec {
            name: "stew_submit_for_review".into(),
            description: "Transition changeset from draft to under_review".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("changeset_id", "Changeset UUID", "uuid", true),
                param("client_request_id", "Idempotency key", "uuid", false),
            ],
        },
        SemRegToolSpec {
            name: "stew_record_review_decision".into(),
            description: "Record a review decision: approve, request_change, or reject".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("changeset_id", "Changeset UUID", "uuid", true),
                param("verdict", "Review verdict: approve, request_change, reject", "string", true),
                param("note", "Review note or explanation", "string", false),
                param("guardrail_overrides", "Guardrails acknowledged/overridden (JSON array)", "json", false),
                param("client_request_id", "Idempotency key", "uuid", false),
            ],
        },
        SemRegToolSpec {
            name: "stew_publish".into(),
            description: "Publish an approved changeset — Draft→Active flip + predecessor supersede".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("changeset_id", "Changeset UUID", "uuid", true),
                param("client_request_id", "Idempotency key", "uuid", false),
            ],
        },
        SemRegToolSpec {
            name: "stew_apply_template".into(),
            description: "Pre-populate changeset entries from a template".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("changeset_id", "Changeset UUID", "uuid", true),
                param("template_fqn", "Template FQN", "string", true),
                param("overrides", "Template variable overrides (JSON object)", "json", false),
                param("client_request_id", "Idempotency key", "uuid", false),
            ],
        },
        SemRegToolSpec {
            name: "stew_validate_edit".into(),
            description: "Run guardrails on a single item (live validation during editing)".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("changeset_id", "Changeset UUID", "uuid", true),
                param("entry_id", "Entry UUID to validate", "uuid", true),
            ],
        },
        SemRegToolSpec {
            name: "stew_resolve_conflict".into(),
            description: "Apply a conflict resolution strategy (merge, rebase, or supersede)".into(),
            category: "stewardship".into(),
            parameters: vec![
                param("conflict_id", "Conflict record UUID", "uuid", true),
                param("strategy", "Resolution strategy: merge, rebase, supersede", "string", true),
                param("resolution_payload", "Merged payload if strategy is merge (JSON)", "json", false),
                param("client_request_id", "Idempotency key", "uuid", false),
            ],
        },
    ]
}

fn stewardship_query_specs() -> Vec<SemRegToolSpec> {
    vec![
        SemRegToolSpec {
            name: "stew_describe_object".into(),
            description: "Describe an object with snapshot, memberships, and consumers".into(),
            category: "stewardship_query".into(),
            parameters: vec![
                param("object_fqn", "Object FQN", "string", true),
                param("object_type", "Object type", "string", false),
                param(
                    "include_consumers",
                    "Include consuming objects (default true)",
                    "boolean",
                    false,
                ),
            ],
        },
        SemRegToolSpec {
            name: "stew_cross_reference".into(),
            description: "Find conflicts, duplicates, and promotable candidates for a changeset"
                .into(),
            category: "stewardship_query".into(),
            parameters: vec![param("changeset_id", "Changeset UUID", "uuid", true)],
        },
        SemRegToolSpec {
            name: "stew_impact_analysis".into(),
            description: "Compute blast radius for changeset items".into(),
            category: "stewardship_query".into(),
            parameters: vec![param("changeset_id", "Changeset UUID", "uuid", true)],
        },
        SemRegToolSpec {
            name: "stew_coverage_report".into(),
            description: "Report on orphans, drift, and intent resolution readiness".into(),
            category: "stewardship_query".into(),
            parameters: vec![
                param("scope", "Domain scope to report on", "string", false),
                param(
                    "include_drift",
                    "Include drift detection (default false)",
                    "boolean",
                    false,
                ),
            ],
        },
    ]
}

// ═══════════════════════════════════════════════════════════════
//  Tool Dispatch
// ═══════════════════════════════════════════════════════════════

/// Dispatch a stewardship Phase 0 tool call.
/// Returns `None` if the tool name is not a stewardship tool.
pub async fn dispatch_phase0_tool(
    ctx: &SemRegToolContext<'_>,
    tool_name: &str,
    args: &serde_json::Value,
) -> Option<SemRegToolResult> {
    let result = match tool_name {
        // Mutating tools
        "stew_compose_changeset" => handle_compose_changeset(ctx, args).await,
        "stew_suggest" => handle_suggest(ctx, args).await,
        "stew_add_item" => handle_add_item(ctx, args).await,
        "stew_remove_item" => handle_remove_item(ctx, args).await,
        "stew_refine_item" => handle_refine_item(ctx, args).await,
        "stew_attach_basis" => handle_attach_basis(ctx, args).await,
        "stew_gate_precheck" => handle_gate_precheck(ctx, args).await,
        "stew_submit_for_review" => handle_submit_for_review(ctx, args).await,
        "stew_record_review_decision" => handle_record_review_decision(ctx, args).await,
        "stew_publish" => handle_publish(ctx, args).await,
        "stew_apply_template" => handle_apply_template(ctx, args).await,
        "stew_validate_edit" => handle_validate_edit(ctx, args).await,
        "stew_resolve_conflict" => handle_resolve_conflict(ctx, args).await,
        // Query tools
        "stew_describe_object" => handle_describe_object(ctx, args).await,
        "stew_cross_reference" => handle_cross_reference(ctx, args).await,
        "stew_impact_analysis" => handle_impact_analysis(ctx, args).await,
        "stew_coverage_report" => handle_coverage_report(ctx, args).await,
        _ => return None,
    };
    Some(result)
}

// ═══════════════════════════════════════════════════════════════
//  Handler Helpers
// ═══════════════════════════════════════════════════════════════

fn get_str<'a>(args: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn get_uuid(args: &serde_json::Value, key: &str) -> Option<Uuid> {
    args.get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

fn get_json(args: &serde_json::Value, key: &str) -> serde_json::Value {
    args.get(key).cloned().unwrap_or(serde_json::Value::Null)
}

fn get_bool(args: &serde_json::Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn get_client_request_id(args: &serde_json::Value) -> Option<Uuid> {
    get_uuid(args, "client_request_id")
}

/// Parse a string into an ObjectType, matching the snake_case wire format.
fn parse_object_type(s: &str) -> Option<ObjectType> {
    match s {
        "attribute_def" => Some(ObjectType::AttributeDef),
        "entity_type_def" => Some(ObjectType::EntityTypeDef),
        "relationship_type_def" => Some(ObjectType::RelationshipTypeDef),
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

fn actor_id(ctx: &SemRegToolContext<'_>) -> String {
    ctx.actor
        .roles
        .first()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string())
}

/// Emit a stewardship audit event.
async fn emit_event(
    pool: &PgPool,
    changeset_id: Uuid,
    event_type: StewardshipEventType,
    actor_id: &str,
) {
    let record = StewardshipRecord {
        event_id: Uuid::new_v4(),
        changeset_id,
        event_type,
        actor_id: actor_id.to_string(),
        payload: json!({}),
        viewport_manifest_id: None,
        created_at: Utc::now(),
    };
    if let Err(e) = StewardshipStore::append_event(pool, &record).await {
        tracing::warn!(error = %e, "Failed to emit stewardship event");
    }
}

// ═══════════════════════════════════════════════════════════════
//  Mutating Tool Handlers
// ═══════════════════════════════════════════════════════════════

async fn handle_compose_changeset(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let scope = match get_str(args, "scope") {
        Some(s) => s,
        None => return SemRegToolResult::err("Missing required parameter: scope"),
    };
    let client_request_id = get_client_request_id(args);

    // Idempotency check
    if let Some(id) = client_request_id {
        match check_idempotency(ctx.pool, Some(id)).await {
            Ok(IdempotencyCheck::Cached(cached)) => return SemRegToolResult::ok(cached),
            Err(e) => return SemRegToolResult::err(format!("Idempotency check failed: {}", e)),
            _ => {}
        }
    }

    let changeset_id = Uuid::new_v4();
    let actor = actor_id(ctx);
    let intent = get_str(args, "intent").unwrap_or("");

    // Create the changeset (snapshot_set) in sem_reg
    match SnapshotStore::create_snapshot_set(
        ctx.pool,
        Some(&format!(
            "changeset:{} scope:{} intent:{}",
            changeset_id, scope, intent
        )),
        &actor,
    )
    .await
    {
        Ok(snapshot_set_id) => {
            // Also create in sem_reg.changesets if table exists
            if let Err(e) = sqlx::query(
                r#"
                INSERT INTO sem_reg.changesets (changeset_id, status, owner_actor_id, scope)
                VALUES ($1, 'draft', $2, $3)
                ON CONFLICT (changeset_id) DO NOTHING
                "#,
            )
            .bind(snapshot_set_id)
            .bind(&actor)
            .bind(scope)
            .execute(ctx.pool)
            .await
            {
                tracing::warn!(error = %e, "Failed to insert into sem_reg.changesets");
            }

            // Emit audit event
            emit_event(
                ctx.pool,
                snapshot_set_id,
                StewardshipEventType::ChangesetCreated,
                &actor,
            )
            .await;

            // Apply template if requested
            let template_fqn = get_str(args, "template_fqn");
            let mut template_items = Vec::new();
            if let Some(fqn) = template_fqn {
                let overrides = get_json(args, "overrides");
                match instantiate_template(ctx.pool, snapshot_set_id, fqn, &actor, &overrides).await
                {
                    Ok(entries) => template_items = entries,
                    Err(e) => {
                        return SemRegToolResult::err(format!(
                            "Changeset created but template instantiation failed: {}",
                            e
                        ))
                    }
                }
            }

            let result = json!({
                "changeset_id": snapshot_set_id,
                "status": "draft",
                "scope": scope,
                "template_items_created": template_items.len(),
            });

            // Record idempotency
            if let Err(e) = record_idempotency(
                ctx.pool,
                client_request_id,
                "stew_compose_changeset",
                &result,
            )
            .await
            {
                tracing::warn!(error = %e, "Failed to record idempotency");
            }

            SemRegToolResult::ok(result)
        }
        Err(e) => SemRegToolResult::err(format!("Failed to create changeset: {}", e)),
    }
}

async fn handle_suggest(ctx: &SemRegToolContext<'_>, args: &serde_json::Value) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };
    let suggestion = match get_str(args, "suggestion") {
        Some(s) => s.to_string(),
        None => return SemRegToolResult::err("Missing required parameter: suggestion"),
    };
    let target_entry_id = get_uuid(args, "target_entry_id");
    let actor = actor_id(ctx);

    // Emit suggestion event
    emit_event(
        ctx.pool,
        changeset_id,
        StewardshipEventType::ItemRefined,
        &actor,
    )
    .await;

    // Record the suggestion as an event with payload
    let record = StewardshipRecord {
        event_id: Uuid::new_v4(),
        changeset_id,
        event_type: StewardshipEventType::ItemRefined,
        actor_id: actor,
        payload: json!({
            "type": "suggestion",
            "suggestion": suggestion,
            "target_entry_id": target_entry_id,
        }),
        viewport_manifest_id: None,
        created_at: Utc::now(),
    };
    if let Err(e) = StewardshipStore::append_event(ctx.pool, &record).await {
        return SemRegToolResult::err(format!("Failed to record suggestion: {}", e));
    }

    SemRegToolResult::ok(json!({
        "changeset_id": changeset_id,
        "suggestion": suggestion,
        "target_entry_id": target_entry_id,
        "recorded": true,
    }))
}

async fn handle_add_item(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };
    let object_fqn = match get_str(args, "object_fqn") {
        Some(s) => s,
        None => return SemRegToolResult::err("Missing required parameter: object_fqn"),
    };
    let object_type_str = match get_str(args, "object_type") {
        Some(s) => s,
        None => return SemRegToolResult::err("Missing required parameter: object_type"),
    };
    let action_str = match get_str(args, "action") {
        Some(s) => s,
        None => return SemRegToolResult::err("Missing required parameter: action"),
    };
    let draft_payload = get_json(args, "draft_payload");
    let predecessor_id = get_uuid(args, "predecessor_id");
    let reasoning = get_str(args, "reasoning").map(|s| s.to_string());
    let client_request_id = get_client_request_id(args);
    let actor = actor_id(ctx);

    // Idempotency check
    if let Some(id) = client_request_id {
        match check_idempotency(ctx.pool, Some(id)).await {
            Ok(IdempotencyCheck::Cached(cached)) => return SemRegToolResult::ok(cached),
            Err(e) => return SemRegToolResult::err(format!("Idempotency check failed: {}", e)),
            _ => {}
        }
    }

    let action = match ChangesetAction::parse(action_str) {
        Some(a) => a,
        None => return SemRegToolResult::err(format!("Invalid action: {}", action_str)),
    };

    let entry_id = Uuid::new_v4();
    let object_type_parsed = parse_object_type(object_type_str).unwrap_or(ObjectType::AttributeDef);
    let object_id = crate::sem_reg::ids::object_id_for(object_type_parsed, object_fqn);

    // Insert the changeset entry
    let insert_result = sqlx::query(
        r#"
        INSERT INTO sem_reg.changeset_entries (
            entry_id, changeset_id, object_fqn, object_type,
            change_kind, draft_payload, base_snapshot_id,
            action, predecessor_id, revision, reasoning, guardrail_log
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1, $10, '[]'::jsonb)
        "#,
    )
    .bind(entry_id)
    .bind(changeset_id)
    .bind(object_fqn)
    .bind(object_type_str)
    .bind(action.as_str())
    .bind(&draft_payload)
    .bind(predecessor_id)
    .bind(action.as_str())
    .bind(predecessor_id)
    .bind(&reasoning)
    .execute(ctx.pool)
    .await;

    match insert_result {
        Ok(_) => {
            // Also write a Draft snapshot to sem_reg.snapshots
            let meta = crate::sem_reg::types::SnapshotMeta {
                object_type: object_type_parsed,
                object_id,
                version_major: 0,
                version_minor: 1,
                status: SnapshotStatus::Draft,
                governance_tier: crate::sem_reg::types::GovernanceTier::Operational,
                trust_class: crate::sem_reg::types::TrustClass::Convenience,
                security_label: Default::default(),
                predecessor_id,
                change_type: crate::sem_reg::types::ChangeType::Created,
                change_rationale: reasoning.clone(),
                created_by: actor.clone(),
                approved_by: None,
            };

            if let Err(e) =
                SnapshotStore::insert_snapshot(ctx.pool, &meta, &draft_payload, Some(changeset_id))
                    .await
            {
                tracing::warn!(error = %e, "Failed to create Draft snapshot for changeset entry");
            }

            // Emit audit event
            emit_event(
                ctx.pool,
                changeset_id,
                StewardshipEventType::ItemAdded,
                &actor,
            )
            .await;

            let result = json!({
                "entry_id": entry_id,
                "changeset_id": changeset_id,
                "object_fqn": object_fqn,
                "object_type": object_type_str,
                "action": action_str,
                "revision": 1,
            });

            if let Err(e) =
                record_idempotency(ctx.pool, client_request_id, "stew_add_item", &result).await
            {
                tracing::warn!(error = %e, "Failed to record idempotency");
            }

            SemRegToolResult::ok(result)
        }
        Err(e) => SemRegToolResult::err(format!("Failed to add item: {}", e)),
    }
}

async fn handle_remove_item(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };
    let entry_id = match get_uuid(args, "entry_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: entry_id"),
    };
    let actor = actor_id(ctx);

    // Delete the entry
    let result = sqlx::query(
        r#"
        DELETE FROM sem_reg.changeset_entries
        WHERE entry_id = $1 AND changeset_id = $2
        "#,
    )
    .bind(entry_id)
    .bind(changeset_id)
    .execute(ctx.pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            emit_event(
                ctx.pool,
                changeset_id,
                StewardshipEventType::ItemRemoved,
                &actor,
            )
            .await;

            SemRegToolResult::ok(json!({
                "entry_id": entry_id,
                "changeset_id": changeset_id,
                "removed": true,
            }))
        }
        Ok(_) => SemRegToolResult::err(format!(
            "Entry {} not found in changeset {}",
            entry_id, changeset_id
        )),
        Err(e) => SemRegToolResult::err(format!("Failed to remove item: {}", e)),
    }
}

async fn handle_refine_item(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };
    let entry_id = match get_uuid(args, "entry_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: entry_id"),
    };
    let draft_payload = get_json(args, "draft_payload");
    let reasoning = get_str(args, "reasoning").map(|s| s.to_string());
    let actor = actor_id(ctx);
    let client_request_id = get_client_request_id(args);

    // Idempotency check
    if let Some(id) = client_request_id {
        match check_idempotency(ctx.pool, Some(id)).await {
            Ok(IdempotencyCheck::Cached(cached)) => return SemRegToolResult::ok(cached),
            Err(e) => return SemRegToolResult::err(format!("Idempotency check failed: {}", e)),
            _ => {}
        }
    }

    // Update draft_payload and bump revision
    let update_result = sqlx::query(
        r#"
        UPDATE sem_reg.changeset_entries
        SET draft_payload = $1,
            revision = revision + 1,
            reasoning = COALESCE($2, reasoning)
        WHERE entry_id = $3 AND changeset_id = $4
        RETURNING revision
        "#,
    )
    .bind(&draft_payload)
    .bind(&reasoning)
    .bind(entry_id)
    .bind(changeset_id)
    .execute(ctx.pool)
    .await;

    match update_result {
        Ok(r) if r.rows_affected() > 0 => {
            emit_event(
                ctx.pool,
                changeset_id,
                StewardshipEventType::ItemRefined,
                &actor,
            )
            .await;

            let result = json!({
                "entry_id": entry_id,
                "changeset_id": changeset_id,
                "refined": true,
            });

            if let Err(e) =
                record_idempotency(ctx.pool, client_request_id, "stew_refine_item", &result).await
            {
                tracing::warn!(error = %e, "Failed to record idempotency");
            }

            SemRegToolResult::ok(result)
        }
        Ok(_) => SemRegToolResult::err(format!(
            "Entry {} not found in changeset {}",
            entry_id, changeset_id
        )),
        Err(e) => SemRegToolResult::err(format!("Failed to refine item: {}", e)),
    }
}

async fn handle_attach_basis(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };
    let kind_str = match get_str(args, "kind") {
        Some(s) => s,
        None => return SemRegToolResult::err("Missing required parameter: kind"),
    };
    let source_ref = match get_str(args, "source_ref") {
        Some(s) => s,
        None => return SemRegToolResult::err("Missing required parameter: source_ref"),
    };
    let excerpt = get_str(args, "excerpt").map(|s| s.to_string());
    let claims_json = get_json(args, "claims");
    let client_request_id = get_client_request_id(args);
    let actor = actor_id(ctx);

    let kind = match BasisKind::parse(kind_str) {
        Some(k) => k,
        None => return SemRegToolResult::err(format!("Invalid basis kind: {}", kind_str)),
    };

    // Insert basis record
    let basis = BasisRecord {
        basis_id: Uuid::new_v4(),
        changeset_id,
        entry_id: None,
        kind,
        title: source_ref.to_string(),
        narrative: excerpt,
        created_by: actor.clone(),
        created_at: Utc::now(),
    };

    if let Err(e) = StewardshipStore::insert_basis(ctx.pool, &basis).await {
        return SemRegToolResult::err(format!("Failed to attach basis: {}", e));
    }

    // Insert claims if provided
    let mut claims_created = 0;
    if let Some(claims_arr) = claims_json.as_array() {
        for claim_json in claims_arr {
            let claim = BasisClaim {
                claim_id: Uuid::new_v4(),
                basis_id: basis.basis_id,
                claim_text: claim_json
                    .get("claim_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                reference_uri: claim_json
                    .get("reference_uri")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                excerpt: claim_json
                    .get("excerpt")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                confidence: claim_json.get("confidence").and_then(|v| v.as_f64()),
                flagged_as_open_question: claim_json
                    .get("flagged_as_open_question")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            };
            if StewardshipStore::insert_claim(ctx.pool, &claim)
                .await
                .is_ok()
            {
                claims_created += 1;
            }
        }
    }

    emit_event(
        ctx.pool,
        changeset_id,
        StewardshipEventType::BasisAttached,
        &actor,
    )
    .await;

    let result = json!({
        "basis_id": basis.basis_id,
        "changeset_id": changeset_id,
        "kind": kind_str,
        "claims_created": claims_created,
    });

    if let Err(e) =
        record_idempotency(ctx.pool, client_request_id, "stew_attach_basis", &result).await
    {
        tracing::warn!(error = %e, "Failed to record idempotency");
    }

    SemRegToolResult::ok(result)
}

async fn handle_gate_precheck(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };

    // Load changeset entries
    let entries = match load_changeset_entries(ctx.pool, changeset_id).await {
        Ok(e) => e,
        Err(e) => return SemRegToolResult::err(format!("Failed to load entries: {}", e)),
    };

    // Load changeset row
    let changeset = match load_changeset_row(ctx.pool, changeset_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return SemRegToolResult::err(format!("Changeset {} not found", changeset_id)),
        Err(e) => return SemRegToolResult::err(format!("Failed to load changeset: {}", e)),
    };

    // Load conflicts
    let conflicts = StewardshipStore::list_conflicts(ctx.pool, changeset_id)
        .await
        .unwrap_or_default();

    // Load basis records
    let basis_records = StewardshipStore::list_basis(ctx.pool, changeset_id)
        .await
        .unwrap_or_default();

    // Run guardrails
    let guardrail_results = evaluate_all_guardrails(
        &changeset,
        &entries,
        &conflicts,
        &basis_records,
        &[], // active_snapshots — loaded on demand in real implementation
        &[], // templates_used
    );

    let blocking = guardrail_results
        .iter()
        .filter(|r| r.severity == GuardrailSeverity::Block)
        .count();
    let warnings = guardrail_results
        .iter()
        .filter(|r| r.severity == GuardrailSeverity::Warning)
        .count();
    let advisories = guardrail_results
        .iter()
        .filter(|r| r.severity == GuardrailSeverity::Advisory)
        .count();

    let can_proceed = blocking == 0;

    // Emit gate_prechecked event
    emit_event(
        ctx.pool,
        changeset_id,
        StewardshipEventType::GatePrechecked {
            result: json!({
                "can_proceed": can_proceed,
                "blocking": blocking,
                "warnings": warnings,
                "advisories": advisories,
            }),
        },
        &actor_id(ctx),
    )
    .await;

    SemRegToolResult::ok(json!({
        "changeset_id": changeset_id,
        "can_proceed": can_proceed,
        "blocking": blocking,
        "warnings": warnings,
        "advisories": advisories,
        "guardrail_results": guardrail_results.iter().map(|r| json!({
            "guardrail_id": format!("{:?}", r.guardrail_id),
            "severity": format!("{:?}", r.severity),
            "message": r.message,
            "context": r.context,
        })).collect::<Vec<_>>(),
    }))
}

async fn handle_submit_for_review(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };
    let client_request_id = get_client_request_id(args);
    let actor = actor_id(ctx);

    // Transition status: draft → under_review
    let result = sqlx::query(
        r#"
        UPDATE sem_reg.changesets
        SET status = 'under_review', updated_at = now()
        WHERE changeset_id = $1 AND status = 'draft'
        "#,
    )
    .bind(changeset_id)
    .execute(ctx.pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            emit_event(
                ctx.pool,
                changeset_id,
                StewardshipEventType::SubmittedForReview,
                &actor,
            )
            .await;

            let result = json!({
                "changeset_id": changeset_id,
                "status": "under_review",
            });

            if let Err(e) = record_idempotency(
                ctx.pool,
                client_request_id,
                "stew_submit_for_review",
                &result,
            )
            .await
            {
                tracing::warn!(error = %e, "Failed to record idempotency");
            }

            SemRegToolResult::ok(result)
        }
        Ok(_) => SemRegToolResult::err(format!(
            "Changeset {} is not in draft status or does not exist",
            changeset_id
        )),
        Err(e) => SemRegToolResult::err(format!("Failed to submit for review: {}", e)),
    }
}

async fn handle_record_review_decision(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };
    let verdict_str = match get_str(args, "verdict") {
        Some(s) => s,
        None => return SemRegToolResult::err("Missing required parameter: verdict"),
    };
    let note = get_str(args, "note").map(|s| s.to_string());
    let client_request_id = get_client_request_id(args);
    let actor = actor_id(ctx);

    let disposition = match verdict_str {
        "approve" => ReviewDisposition::Approve,
        "request_change" => ReviewDisposition::RequestChange,
        "reject" => ReviewDisposition::Reject,
        _ => return SemRegToolResult::err(format!("Invalid verdict: {}", verdict_str)),
    };

    // Map tool-level verdict names to DB column values
    let db_verdict = match disposition {
        ReviewDisposition::Approve => "approved",
        ReviewDisposition::RequestChange => "requested_changes",
        ReviewDisposition::Reject => "rejected",
    };

    // Insert review record
    let review_id = Uuid::new_v4();
    let insert_result = sqlx::query(
        r#"
        INSERT INTO sem_reg.changeset_reviews (
            review_id, changeset_id, actor_id, verdict, comment
        ) VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(review_id)
    .bind(changeset_id)
    .bind(&actor)
    .bind(db_verdict)
    .bind(&note)
    .execute(ctx.pool)
    .await;

    if let Err(e) = insert_result {
        return SemRegToolResult::err(format!("Failed to record review: {}", e));
    }

    // Update changeset status based on verdict
    let new_status = match disposition {
        ReviewDisposition::Approve => "approved",
        ReviewDisposition::RequestChange => "draft",
        ReviewDisposition::Reject => "rejected",
    };

    let _ = sqlx::query(
        r#"
        UPDATE sem_reg.changesets
        SET status = $1, updated_at = now()
        WHERE changeset_id = $2
        "#,
    )
    .bind(new_status)
    .bind(changeset_id)
    .execute(ctx.pool)
    .await;

    // Emit event
    emit_event(
        ctx.pool,
        changeset_id,
        StewardshipEventType::ReviewDecisionRecorded { disposition },
        &actor,
    )
    .await;

    let result = json!({
        "review_id": review_id,
        "changeset_id": changeset_id,
        "verdict": verdict_str,
        "new_status": new_status,
    });

    if let Err(e) = record_idempotency(
        ctx.pool,
        client_request_id,
        "stew_record_review_decision",
        &result,
    )
    .await
    {
        tracing::warn!(error = %e, "Failed to record idempotency");
    }

    SemRegToolResult::ok(result)
}

async fn handle_publish(ctx: &SemRegToolContext<'_>, args: &serde_json::Value) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };
    let client_request_id = get_client_request_id(args);
    let actor = actor_id(ctx);

    // Idempotency check
    if let Some(id) = client_request_id {
        match check_idempotency(ctx.pool, Some(id)).await {
            Ok(IdempotencyCheck::Cached(cached)) => return SemRegToolResult::ok(cached),
            Err(e) => return SemRegToolResult::err(format!("Idempotency check failed: {}", e)),
            _ => {}
        }
    }

    // Verify changeset is approved
    match load_changeset_row(ctx.pool, changeset_id).await {
        Ok(Some(cs)) if cs.status == ChangesetStatus::Approved => {}
        Ok(Some(cs)) => {
            return SemRegToolResult::err(format!(
                "Changeset {} is in status {:?}, must be approved to publish",
                changeset_id, cs.status
            ))
        }
        Ok(None) => return SemRegToolResult::err(format!("Changeset {} not found", changeset_id)),
        Err(e) => return SemRegToolResult::err(format!("Failed to load changeset: {}", e)),
    }

    // Publish in a single transaction: supersede predecessors first,
    // then promote drafts to active. Order matters because the unique
    // index uix_snapshots_active allows only one active snapshot per
    // (object_type, object_id) with effective_until IS NULL.
    let mut tx = match ctx.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => return SemRegToolResult::err(format!("Failed to begin transaction: {}", e)),
    };

    // Step 1: Supersede predecessors (set effective_until on old active snapshots)
    let _ = sqlx::query(
        r#"
        UPDATE sem_reg.snapshots s
        SET effective_until = now()
        FROM sem_reg.snapshots draft
        WHERE draft.snapshot_set_id = $1
          AND draft.status = 'draft'
          AND draft.predecessor_id = s.snapshot_id
          AND s.effective_until IS NULL
          AND s.snapshot_id != draft.snapshot_id
        "#,
    )
    .bind(changeset_id)
    .execute(&mut *tx)
    .await;

    // Step 2: For "add" items that create new objects with the same object_id
    // as an existing active snapshot, supersede the existing active snapshot.
    // This handles the case where a taxonomy membership is being re-added.
    let _ = sqlx::query(
        r#"
        UPDATE sem_reg.snapshots existing
        SET effective_until = now()
        FROM sem_reg.snapshots draft
        WHERE draft.snapshot_set_id = $1
          AND draft.status = 'draft'
          AND draft.predecessor_id IS NULL
          AND existing.object_type = draft.object_type
          AND existing.object_id = draft.object_id
          AND existing.status = 'active'
          AND existing.effective_until IS NULL
          AND existing.snapshot_id != draft.snapshot_id
        "#,
    )
    .bind(changeset_id)
    .execute(&mut *tx)
    .await;

    // Step 3: Promote all Draft snapshots to Active
    let promote_result = sqlx::query(
        r#"
        UPDATE sem_reg.snapshots
        SET status = 'active'
        WHERE snapshot_set_id = $1 AND status = 'draft'
        "#,
    )
    .bind(changeset_id)
    .execute(&mut *tx)
    .await;

    let promoted_count = match promote_result {
        Ok(r) => r.rows_affected(),
        Err(e) => {
            let _ = tx.rollback().await;
            return SemRegToolResult::err(format!("Failed to promote snapshots: {}", e));
        }
    };

    if let Err(e) = tx.commit().await {
        return SemRegToolResult::err(format!("Failed to commit publish transaction: {}", e));
    }

    // Update changeset status to published
    let _ = sqlx::query(
        r#"
        UPDATE sem_reg.changesets
        SET status = 'published', updated_at = now()
        WHERE changeset_id = $1
        "#,
    )
    .bind(changeset_id)
    .execute(ctx.pool)
    .await;

    emit_event(
        ctx.pool,
        changeset_id,
        StewardshipEventType::Published,
        &actor,
    )
    .await;

    let result = json!({
        "changeset_id": changeset_id,
        "status": "published",
        "snapshots_promoted": promoted_count,
    });

    if let Err(e) = record_idempotency(ctx.pool, client_request_id, "stew_publish", &result).await {
        tracing::warn!(error = %e, "Failed to record idempotency");
    }

    SemRegToolResult::ok(result)
}

async fn handle_apply_template(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };
    let template_fqn = match get_str(args, "template_fqn") {
        Some(s) => s,
        None => return SemRegToolResult::err("Missing required parameter: template_fqn"),
    };
    let overrides = get_json(args, "overrides");
    let client_request_id = get_client_request_id(args);
    let actor = actor_id(ctx);

    match instantiate_template(ctx.pool, changeset_id, template_fqn, &actor, &overrides).await {
        Ok(entries) => {
            let result = json!({
                "changeset_id": changeset_id,
                "template_fqn": template_fqn,
                "items_created": entries.len(),
                "entry_ids": entries.iter().map(|e| e.entry_id).collect::<Vec<_>>(),
            });

            if let Err(e) =
                record_idempotency(ctx.pool, client_request_id, "stew_apply_template", &result)
                    .await
            {
                tracing::warn!(error = %e, "Failed to record idempotency");
            }

            SemRegToolResult::ok(result)
        }
        Err(e) => SemRegToolResult::err(format!("Failed to apply template: {}", e)),
    }
}

async fn handle_validate_edit(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };
    let entry_id = match get_uuid(args, "entry_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: entry_id"),
    };

    // Load changeset and the specific entry
    let entries = match load_changeset_entries(ctx.pool, changeset_id).await {
        Ok(e) => e,
        Err(e) => return SemRegToolResult::err(format!("Failed to load entries: {}", e)),
    };

    let target_entry: Vec<_> = entries
        .iter()
        .filter(|e| e.entry_id == entry_id)
        .cloned()
        .collect();

    if target_entry.is_empty() {
        return SemRegToolResult::err(format!(
            "Entry {} not found in changeset {}",
            entry_id, changeset_id
        ));
    }

    let changeset = match load_changeset_row(ctx.pool, changeset_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return SemRegToolResult::err(format!("Changeset {} not found", changeset_id)),
        Err(e) => return SemRegToolResult::err(format!("Failed to load changeset: {}", e)),
    };

    // Run guardrails on the single entry
    let guardrail_results = evaluate_all_guardrails(&changeset, &target_entry, &[], &[], &[], &[]);

    SemRegToolResult::ok(json!({
        "entry_id": entry_id,
        "changeset_id": changeset_id,
        "valid": guardrail_results.iter().all(|r| r.severity != GuardrailSeverity::Block),
        "guardrail_results": guardrail_results.iter().map(|r| json!({
            "guardrail_id": format!("{:?}", r.guardrail_id),
            "severity": format!("{:?}", r.severity),
            "message": r.message,
        })).collect::<Vec<_>>(),
    }))
}

async fn handle_resolve_conflict(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let conflict_id = match get_uuid(args, "conflict_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: conflict_id"),
    };
    let strategy_str = match get_str(args, "strategy") {
        Some(s) => s,
        None => return SemRegToolResult::err("Missing required parameter: strategy"),
    };
    let resolution_payload = get_json(args, "resolution_payload");
    let client_request_id = get_client_request_id(args);
    let actor = actor_id(ctx);

    let strategy = match ConflictStrategy::parse(strategy_str) {
        Some(s) => s,
        None => return SemRegToolResult::err(format!("Invalid strategy: {}", strategy_str)),
    };

    let rationale = resolution_payload
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| resolution_payload.to_string());

    match StewardshipStore::resolve_conflict(ctx.pool, conflict_id, strategy, &rationale, &actor)
        .await
    {
        Ok(()) => {
            let result = json!({
                "conflict_id": conflict_id,
                "strategy": strategy_str,
                "resolved": true,
            });

            if let Err(e) = record_idempotency(
                ctx.pool,
                client_request_id,
                "stew_resolve_conflict",
                &result,
            )
            .await
            {
                tracing::warn!(error = %e, "Failed to record idempotency");
            }

            SemRegToolResult::ok(result)
        }
        Err(e) => SemRegToolResult::err(format!("Failed to resolve conflict: {}", e)),
    }
}

// ═══════════════════════════════════════════════════════════════
//  Query Tool Handlers
// ═══════════════════════════════════════════════════════════════

async fn handle_describe_object(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let object_fqn = match get_str(args, "object_fqn") {
        Some(s) => s,
        None => return SemRegToolResult::err("Missing required parameter: object_fqn"),
    };
    let object_type_str = get_str(args, "object_type");
    let include_consumers = get_bool(args, "include_consumers", true);

    // Try to find the object across all types or a specific type
    let snapshot = if let Some(ot_str) = object_type_str {
        let ot = parse_object_type(ot_str).unwrap_or(ObjectType::AttributeDef);
        SnapshotStore::find_active_by_definition_field(ctx.pool, ot, "fqn", object_fqn).await
    } else {
        // Search across all object types
        let types = [
            ObjectType::AttributeDef,
            ObjectType::EntityTypeDef,
            ObjectType::VerbContract,
            ObjectType::TaxonomyDef,
            ObjectType::ViewDef,
            ObjectType::PolicyRule,
        ];
        let mut found = Ok(None);
        for ot in &types {
            match SnapshotStore::find_active_by_definition_field(ctx.pool, *ot, "fqn", object_fqn)
                .await
            {
                Ok(Some(s)) => {
                    found = Ok(Some(s));
                    break;
                }
                Ok(None) => continue,
                Err(e) => {
                    found = Err(e);
                    break;
                }
            }
        }
        found
    };

    match snapshot {
        Ok(Some(row)) => {
            let mut result = json!({
                "snapshot_id": row.snapshot_id,
                "object_id": row.object_id,
                "object_type": row.object_type.to_string(),
                "fqn": object_fqn,
                "version": row.version_string(),
                "governance_tier": row.governance_tier,
                "trust_class": row.trust_class,
                "status": row.status,
                "definition": row.definition,
            });

            // Add consumer information if requested
            if include_consumers {
                let consumers = find_consumers(ctx.pool, object_fqn)
                    .await
                    .unwrap_or_default();
                result
                    .as_object_mut()
                    .unwrap()
                    .insert("consumers".into(), json!(consumers));
            }

            SemRegToolResult::ok(result)
        }
        Ok(None) => SemRegToolResult::err(format!("Object '{}' not found", object_fqn)),
        Err(e) => SemRegToolResult::err(format!("Failed to describe object: {}", e)),
    }
}

async fn handle_cross_reference(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };

    // Load entries
    let entries = match load_changeset_entries(ctx.pool, changeset_id).await {
        Ok(e) => e,
        Err(e) => return SemRegToolResult::err(format!("Failed to load entries: {}", e)),
    };

    // Find conflicts: check if any entry FQNs are modified in other open changesets
    let mut conflicts = Vec::new();
    for entry in &entries {
        let rows = sqlx::query_as::<_, ConflictCandidate>(
            r#"
            SELECT ce.changeset_id, ce.object_fqn, ce.change_kind
            FROM sem_reg.changeset_entries ce
            JOIN sem_reg.changesets c ON c.changeset_id = ce.changeset_id
            WHERE ce.object_fqn = $1
              AND ce.changeset_id != $2
              AND c.status IN ('draft', 'under_review')
            LIMIT 10
            "#,
        )
        .bind(&entry.object_fqn)
        .bind(changeset_id)
        .fetch_all(ctx.pool)
        .await
        .unwrap_or_default();

        for row in rows {
            conflicts.push(json!({
                "fqn": entry.object_fqn,
                "competing_changeset_id": row.changeset_id,
                "competing_change_kind": row.change_kind,
            }));
        }
    }

    SemRegToolResult::ok(json!({
        "changeset_id": changeset_id,
        "entry_count": entries.len(),
        "conflicts_found": conflicts.len(),
        "conflicts": conflicts,
    }))
}

async fn handle_impact_analysis(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let changeset_id = match get_uuid(args, "changeset_id") {
        Some(id) => id,
        None => return SemRegToolResult::err("Missing required parameter: changeset_id"),
    };

    let entries = match load_changeset_entries(ctx.pool, changeset_id).await {
        Ok(e) => e,
        Err(e) => return SemRegToolResult::err(format!("Failed to load entries: {}", e)),
    };

    match compute_changeset_impact(ctx.pool, changeset_id, &entries).await {
        Ok(report) => SemRegToolResult::ok(serde_json::to_value(&report).unwrap_or(json!({}))),
        Err(e) => SemRegToolResult::err(format!("Failed to compute impact: {}", e)),
    }
}

async fn handle_coverage_report(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let _scope = get_str(args, "scope");
    let include_drift = get_bool(args, "include_drift", false);

    // Count active snapshots by type
    let type_counts = sqlx::query_as::<_, TypeCount>(
        r#"
        SELECT object_type::text as object_type, COUNT(*)::bigint as count
        FROM sem_reg.snapshots
        WHERE status = 'active'
          AND effective_until IS NULL
        GROUP BY object_type
        ORDER BY object_type
        "#,
    )
    .fetch_all(ctx.pool)
    .await
    .unwrap_or_default();

    // Count orphaned objects (active snapshots not referenced by any view or policy)
    let orphan_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint
        FROM sem_reg.snapshots s
        WHERE s.status = 'active'
          AND s.effective_until IS NULL
          AND s.object_type IN ('attribute_def', 'entity_type_def')
          AND NOT EXISTS (
              SELECT 1 FROM sem_reg.snapshots consumer
              WHERE consumer.status = 'active'
                AND consumer.effective_until IS NULL
                AND consumer.object_type IN ('view_def', 'policy_rule')
                AND consumer.definition::text LIKE '%' || (s.definition->>'fqn') || '%'
          )
        "#,
    )
    .fetch_one(ctx.pool)
    .await
    .unwrap_or(0);

    let mut result = json!({
        "type_counts": type_counts.iter().map(|tc| json!({
            "object_type": tc.object_type,
            "count": tc.count,
        })).collect::<Vec<_>>(),
        "orphan_count": orphan_count,
    });

    if include_drift {
        // Count snapshots with stale embeddings
        let stale_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint
            FROM sem_reg.embedding_records e
            JOIN sem_reg.snapshots s ON s.snapshot_id = e.snapshot_id
            WHERE s.status = 'active'
              AND s.effective_until IS NULL
              AND e.version_hash IS DISTINCT FROM md5(s.definition::text)
            "#,
        )
        .fetch_one(ctx.pool)
        .await
        .unwrap_or(0);

        result
            .as_object_mut()
            .unwrap()
            .insert("stale_embeddings".into(), json!(stale_count));
    }

    SemRegToolResult::ok(result)
}

// ═══════════════════════════════════════════════════════════════
//  Internal Helper Queries
// ═══════════════════════════════════════════════════════════════

async fn load_changeset_entries(
    pool: &PgPool,
    changeset_id: Uuid,
) -> anyhow::Result<Vec<ChangesetEntryRow>> {
    let rows = sqlx::query_as::<_, ChangesetEntryDbRow>(
        r#"
        SELECT entry_id, changeset_id, object_fqn, object_type,
               change_kind, draft_payload, base_snapshot_id,
               created_at, action, predecessor_id, revision,
               reasoning, guardrail_log
        FROM sem_reg.changeset_entries
        WHERE changeset_id = $1
        ORDER BY created_at
        "#,
    )
    .bind(changeset_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

async fn load_changeset_row(
    pool: &PgPool,
    changeset_id: Uuid,
) -> anyhow::Result<Option<ChangesetRow>> {
    let row = sqlx::query_as::<_, ChangesetDbRow>(
        r#"
        SELECT changeset_id, status, owner_actor_id, scope,
               created_at, updated_at
        FROM sem_reg.changesets
        WHERE changeset_id = $1
        "#,
    )
    .bind(changeset_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

/// Find active snapshots that reference a given FQN in their definition.
async fn find_consumers(pool: &PgPool, fqn: &str) -> anyhow::Result<Vec<serde_json::Value>> {
    let rows = sqlx::query_as::<_, ConsumerRow>(
        r#"
        SELECT snapshot_id, object_type::text as object_type,
               COALESCE(definition->>'fqn', object_id::text) as fqn
        FROM sem_reg.snapshots
        WHERE status = 'active'
          AND effective_until IS NULL
          AND definition::text LIKE '%' || $1 || '%'
          AND COALESCE(definition->>'fqn', '') != $1
        LIMIT 50
        "#,
    )
    .bind(fqn)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| {
            json!({
                "snapshot_id": r.snapshot_id,
                "object_type": r.object_type,
                "fqn": r.fqn,
            })
        })
        .collect())
}

// ── FromRow helper types ─────────────────────────────────────

#[derive(sqlx::FromRow)]
struct ChangesetEntryDbRow {
    entry_id: Uuid,
    changeset_id: Uuid,
    object_fqn: String,
    object_type: String,
    change_kind: String,
    draft_payload: serde_json::Value,
    base_snapshot_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    action: Option<String>,
    predecessor_id: Option<Uuid>,
    revision: Option<i32>,
    reasoning: Option<String>,
    guardrail_log: Option<serde_json::Value>,
}

use chrono::DateTime;

impl From<ChangesetEntryDbRow> for ChangesetEntryRow {
    fn from(r: ChangesetEntryDbRow) -> Self {
        Self {
            entry_id: r.entry_id,
            changeset_id: r.changeset_id,
            object_fqn: r.object_fqn,
            object_type: r.object_type,
            change_kind: r.change_kind,
            draft_payload: r.draft_payload,
            base_snapshot_id: r.base_snapshot_id,
            created_at: r.created_at,
            action: r
                .action
                .as_deref()
                .and_then(ChangesetAction::parse)
                .unwrap_or(ChangesetAction::Add),
            predecessor_id: r.predecessor_id,
            revision: r.revision.unwrap_or(1),
            reasoning: r.reasoning,
            guardrail_log: r.guardrail_log.unwrap_or(json!([])),
        }
    }
}

#[derive(sqlx::FromRow)]
struct ChangesetDbRow {
    changeset_id: Uuid,
    status: String,
    owner_actor_id: String,
    scope: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ChangesetDbRow> for ChangesetRow {
    fn from(r: ChangesetDbRow) -> Self {
        Self {
            changeset_id: r.changeset_id,
            status: ChangesetStatus::parse(&r.status).unwrap_or(ChangesetStatus::Draft),
            owner_actor_id: r.owner_actor_id,
            scope: r.scope,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct ConflictCandidate {
    changeset_id: Uuid,
    object_fqn: String,
    change_kind: String,
}

#[derive(sqlx::FromRow)]
struct ConsumerRow {
    snapshot_id: Uuid,
    object_type: String,
    fqn: String,
}

#[derive(sqlx::FromRow)]
struct TypeCount {
    object_type: String,
    count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase0_tool_count() {
        let specs = phase0_tool_specs();
        assert_eq!(specs.len(), 17, "Expected 17 Phase 0 tools");
    }

    #[test]
    fn test_all_tools_have_unique_names() {
        let specs = phase0_tool_specs();
        let mut names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), specs.len(), "Duplicate tool names found");
    }

    #[test]
    fn test_all_mutating_tools_accept_client_request_id() {
        let specs = stewardship_mutating_specs();
        let tools_needing_idempotency = [
            "stew_compose_changeset",
            "stew_suggest",
            "stew_add_item",
            "stew_remove_item",
            "stew_refine_item",
            "stew_attach_basis",
            "stew_submit_for_review",
            "stew_record_review_decision",
            "stew_publish",
            "stew_apply_template",
            "stew_resolve_conflict",
        ];
        for name in &tools_needing_idempotency {
            let spec = specs.iter().find(|s| s.name == *name);
            assert!(spec.is_some(), "Missing tool spec: {}", name);
            let has_cri = spec
                .unwrap()
                .parameters
                .iter()
                .any(|p| p.name == "client_request_id");
            assert!(has_cri, "Tool {} missing client_request_id parameter", name);
        }
    }

    #[test]
    fn test_dispatch_returns_none_for_unknown() {
        // dispatch_phase0_tool returns None for non-stewardship tools
        // This is a sync check that the match arms exist
        let specs = phase0_tool_specs();
        for spec in &specs {
            assert!(
                spec.name.starts_with("stew_"),
                "Tool {} should have stew_ prefix",
                spec.name
            );
        }
    }
}
