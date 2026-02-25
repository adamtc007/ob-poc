//! Phase 1 MCP tools — 6 visualisation tools for the Show Loop.
//!
//! Spec §6.4 Visualisation tools:
//!   stew_get_focus      — Get current FocusState for session
//!   stew_set_focus      — Set FocusState (emits FocusChanged event)
//!   stew_show           — Trigger ShowPacket computation
//!   stew_get_viewport   — Compute single viewport model by kind
//!   stew_get_diff       — Structured diff: predecessor vs draft
//!   stew_capture_manifest — Compute + persist ViewportManifest

use serde_json::json;
use uuid::Uuid;

use crate::sem_reg::agent::mcp_tools::{
    SemRegToolContext, SemRegToolResult, SemRegToolSpec, ToolParameter,
};
use crate::sem_reg::stewardship::focus::FocusStore;
use crate::sem_reg::stewardship::show_loop::ShowLoop;
use crate::sem_reg::stewardship::types::*;

/// Return all Phase 1 tool specs (6 tools).
pub fn phase1_tool_specs() -> Vec<SemRegToolSpec> {
    vec![
        SemRegToolSpec {
            name: "stew_get_focus".into(),
            description: "Get the current FocusState for a session.".into(),
            category: "visualisation".into(),
            parameters: vec![ToolParameter {
                name: "session_id".into(),
                description: "Session UUID".into(),
                param_type: "string".into(),
                required: true,
            }],
        },
        SemRegToolSpec {
            name: "stew_set_focus".into(),
            description: "Set the FocusState for a session. Emits FocusChanged audit event.".into(),
            category: "visualisation".into(),
            parameters: vec![
                ToolParameter {
                    name: "session_id".into(),
                    description: "Session UUID".into(),
                    param_type: "string".into(),
                    required: true,
                },
                ToolParameter {
                    name: "changeset_id".into(),
                    description: "Changeset UUID".into(),
                    param_type: "string".into(),
                    required: false,
                },
                ToolParameter {
                    name: "overlay_mode".into(),
                    description: "Overlay mode: active_only or draft_overlay".into(),
                    param_type: "string".into(),
                    required: false,
                },
                ToolParameter {
                    name: "object_refs".into(),
                    description: "Array of object references to focus on".into(),
                    param_type: "array".into(),
                    required: false,
                },
                ToolParameter {
                    name: "taxonomy_fqn".into(),
                    description: "Taxonomy FQN to focus on".into(),
                    param_type: "string".into(),
                    required: false,
                },
                ToolParameter {
                    name: "taxonomy_node_id".into(),
                    description: "Taxonomy node ID within the taxonomy".into(),
                    param_type: "string".into(),
                    required: false,
                },
                ToolParameter {
                    name: "source".into(),
                    description: "Update source: agent or user_navigation".into(),
                    param_type: "string".into(),
                    required: false,
                },
            ],
        },
        SemRegToolSpec {
            name: "stew_show".into(),
            description:
                "Trigger ShowPacket computation and return the full packet with all viewports."
                    .into(),
            category: "visualisation".into(),
            parameters: vec![
                ToolParameter {
                    name: "session_id".into(),
                    description: "Session UUID".into(),
                    param_type: "string".into(),
                    required: true,
                },
                ToolParameter {
                    name: "assume_principal".into(),
                    description: "Principal to assume for access control".into(),
                    param_type: "string".into(),
                    required: false,
                },
            ],
        },
        SemRegToolSpec {
            name: "stew_get_viewport".into(),
            description: "Compute a single viewport model by kind (focus, object, diff, gates)."
                .into(),
            category: "visualisation".into(),
            parameters: vec![
                ToolParameter {
                    name: "session_id".into(),
                    description: "Session UUID".into(),
                    param_type: "string".into(),
                    required: true,
                },
                ToolParameter {
                    name: "viewport_kind".into(),
                    description: "Viewport kind: focus, object, diff, or gates".into(),
                    param_type: "string".into(),
                    required: true,
                },
            ],
        },
        SemRegToolSpec {
            name: "stew_get_diff".into(),
            description:
                "Get structured diff between predecessor Active and Draft successor for a changeset."
                    .into(),
            category: "visualisation".into(),
            parameters: vec![ToolParameter {
                name: "session_id".into(),
                description: "Session UUID".into(),
                param_type: "string".into(),
                required: true,
            }],
        },
        SemRegToolSpec {
            name: "stew_capture_manifest".into(),
            description:
                "Compute and persist a ViewportManifest with SHA-256 hashes for audit."
                    .into(),
            category: "visualisation".into(),
            parameters: vec![
                ToolParameter {
                    name: "session_id".into(),
                    description: "Session UUID".into(),
                    param_type: "string".into(),
                    required: true,
                },
                ToolParameter {
                    name: "assume_principal".into(),
                    description: "Principal to assume for access control".into(),
                    param_type: "string".into(),
                    required: false,
                },
            ],
        },
    ]
}

/// Dispatch a Phase 1 tool call. Returns None if tool name not recognized.
pub async fn dispatch_phase1_tool(
    ctx: &SemRegToolContext<'_>,
    tool_name: &str,
    args: &serde_json::Value,
) -> Option<SemRegToolResult> {
    match tool_name {
        "stew_get_focus" => Some(handle_get_focus(ctx, args).await),
        "stew_set_focus" => Some(handle_set_focus(ctx, args).await),
        "stew_show" => Some(handle_show(ctx, args).await),
        "stew_get_viewport" => Some(handle_get_viewport(ctx, args).await),
        "stew_get_diff" => Some(handle_get_diff(ctx, args).await),
        "stew_capture_manifest" => Some(handle_capture_manifest(ctx, args).await),
        _ => None,
    }
}

// ─── Tool handlers ──────────────────────────────────────────────

async fn handle_get_focus(ctx: &SemRegToolContext<'_>, args: &serde_json::Value) -> SemRegToolResult {
    let session_id = match parse_uuid_arg(args, "session_id") {
        Ok(id) => id,
        Err(e) => return SemRegToolResult::err(e),
    };

    match FocusStore::get(&ctx.pool, session_id).await {
        Ok(Some(focus)) => match serde_json::to_value(&focus) {
            Ok(v) => SemRegToolResult::ok(v),
            Err(e) => SemRegToolResult::err(format!("Serialization error: {e}")),
        },
        Ok(None) => SemRegToolResult::ok(json!({
            "message": "No focus state found for this session",
            "session_id": session_id.to_string(),
        })),
        Err(e) => SemRegToolResult::err(format!("Failed to get focus: {e}")),
    }
}

async fn handle_set_focus(ctx: &SemRegToolContext<'_>, args: &serde_json::Value) -> SemRegToolResult {
    let session_id = match parse_uuid_arg(args, "session_id") {
        Ok(id) => id,
        Err(e) => return SemRegToolResult::err(e),
    };

    let changeset_id = parse_optional_uuid_arg(args, "changeset_id");

    let overlay_mode = match args.get("overlay_mode").and_then(|v| v.as_str()) {
        Some("draft_overlay") => match changeset_id {
            Some(cs_id) => OverlayMode::DraftOverlay {
                changeset_id: cs_id,
            },
            None => {
                return SemRegToolResult::err(
                    "draft_overlay mode requires changeset_id".to_string(),
                )
            }
        },
        _ => OverlayMode::ActiveOnly,
    };

    let object_refs: Vec<ObjectRef> = args
        .get("object_refs")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let taxonomy_focus = args
        .get("taxonomy_fqn")
        .and_then(|v| v.as_str())
        .map(|fqn| TaxonomyFocus {
            taxonomy_fqn: fqn.to_string(),
            node_id: args
                .get("taxonomy_node_id")
                .and_then(|v| v.as_str())
                .map(String::from),
        });

    let source = match args.get("source").and_then(|v| v.as_str()) {
        Some("user_navigation") => FocusUpdateSource::UserNavigation,
        _ => FocusUpdateSource::Agent,
    };

    let focus = FocusState {
        session_id,
        changeset_id,
        overlay_mode,
        object_refs,
        taxonomy_focus,
        resolution_context: None,
        updated_at: chrono::Utc::now(),
        updated_by: source.clone(),
    };

    match FocusStore::set(&ctx.pool, &focus, source, changeset_id).await {
        Ok(()) => SemRegToolResult::ok(json!({
            "status": "focus_set",
            "session_id": session_id.to_string(),
        })),
        Err(e) => SemRegToolResult::err(format!("Failed to set focus: {e}")),
    }
}

async fn handle_show(ctx: &SemRegToolContext<'_>, args: &serde_json::Value) -> SemRegToolResult {
    let session_id = match parse_uuid_arg(args, "session_id") {
        Ok(id) => id,
        Err(e) => return SemRegToolResult::err(e),
    };

    let assume_principal = args
        .get("assume_principal")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Load current focus
    let focus = match FocusStore::get(&ctx.pool, session_id).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return SemRegToolResult::err(format!(
                "No focus state for session {session_id}. Use stew_set_focus first."
            ))
        }
        Err(e) => return SemRegToolResult::err(format!("Failed to load focus: {e}")),
    };

    // Compute ShowPacket
    match ShowLoop::compute_show_packet(
        &ctx.pool,
        &focus,
        &ctx.actor.actor_id,
        assume_principal.as_deref(),
    )
    .await
    {
        Ok(packet) => match serde_json::to_value(&packet) {
            Ok(v) => SemRegToolResult::ok(v),
            Err(e) => SemRegToolResult::err(format!("Serialization error: {e}")),
        },
        Err(e) => SemRegToolResult::err(format!("Failed to compute ShowPacket: {e}")),
    }
}

async fn handle_get_viewport(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let session_id = match parse_uuid_arg(args, "session_id") {
        Ok(id) => id,
        Err(e) => return SemRegToolResult::err(e),
    };

    let viewport_kind = match args.get("viewport_kind").and_then(|v| v.as_str()) {
        Some(k) => k,
        None => return SemRegToolResult::err("Missing required parameter: viewport_kind"),
    };

    // Load current focus
    let focus = match FocusStore::get(&ctx.pool, session_id).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return SemRegToolResult::err(format!(
                "No focus state for session {session_id}. Use stew_set_focus first."
            ))
        }
        Err(e) => return SemRegToolResult::err(format!("Failed to load focus: {e}")),
    };

    // Compute the requested viewport
    let result = match viewport_kind {
        "focus" => ShowLoop::compute_show_packet(&ctx.pool, &focus, &ctx.actor.actor_id, None)
            .await
            .map(|p| {
                p.viewports
                    .iter()
                    .find(|v| v.kind == ViewportKind::Focus)
                    .map(|_| json!({"viewport": "focus", "status": "ready"}))
                    .unwrap_or(json!({"error": "Focus viewport not available"}))
            }),
        "object" => ShowLoop::compute_show_packet(&ctx.pool, &focus, &ctx.actor.actor_id, None)
            .await
            .map(|p| {
                p.viewports
                    .iter()
                    .find(|v| v.kind == ViewportKind::Object)
                    .map(|_| json!({"viewport": "object", "status": "ready"}))
                    .unwrap_or(json!({"error": "Object viewport not available — no objects in focus"}))
            }),
        "diff" => ShowLoop::compute_show_packet(&ctx.pool, &focus, &ctx.actor.actor_id, None)
            .await
            .map(|p| {
                p.viewports
                    .iter()
                    .find(|v| v.kind == ViewportKind::Diff)
                    .map(|_| json!({"viewport": "diff", "status": "ready"}))
                    .unwrap_or(json!({"error": "Diff viewport not available — no draft overlay active"}))
            }),
        "gates" => ShowLoop::compute_show_packet(&ctx.pool, &focus, &ctx.actor.actor_id, None)
            .await
            .map(|p| {
                p.viewports
                    .iter()
                    .find(|v| v.kind == ViewportKind::Gates)
                    .map(|_| json!({"viewport": "gates", "status": "ready"}))
                    .unwrap_or(json!({"error": "Gates viewport not available — no changeset selected"}))
            }),
        other => {
            return SemRegToolResult::err(format!(
                "Unknown viewport kind: {other}. Valid kinds: focus, object, diff, gates"
            ))
        }
    };

    match result {
        Ok(v) => SemRegToolResult::ok(v),
        Err(e) => SemRegToolResult::err(format!("Failed to compute viewport: {e}")),
    }
}

async fn handle_get_diff(ctx: &SemRegToolContext<'_>, args: &serde_json::Value) -> SemRegToolResult {
    let session_id = match parse_uuid_arg(args, "session_id") {
        Ok(id) => id,
        Err(e) => return SemRegToolResult::err(e),
    };

    // Load current focus
    let focus = match FocusStore::get(&ctx.pool, session_id).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return SemRegToolResult::err(format!(
                "No focus state for session {session_id}. Use stew_set_focus first."
            ))
        }
        Err(e) => return SemRegToolResult::err(format!("Failed to load focus: {e}")),
    };

    if !matches!(focus.overlay_mode, OverlayMode::DraftOverlay { .. }) {
        return SemRegToolResult::ok(json!({
            "message": "No draft overlay active. Enable draft overlay first with stew_set_focus.",
            "diffs": [],
        }));
    }

    // Compute full ShowPacket and extract diff viewport data
    match ShowLoop::compute_show_packet(&ctx.pool, &focus, &ctx.actor.actor_id, None).await {
        Ok(packet) => {
            let diff_data = packet
                .viewports
                .iter()
                .find(|v| v.kind == ViewportKind::Diff)
                .map(|v| json!({"viewport_id": v.id, "diff_data": v.params}))
                .unwrap_or(json!({"message": "Diff viewport not computed"}));
            SemRegToolResult::ok(diff_data)
        }
        Err(e) => SemRegToolResult::err(format!("Failed to compute diff: {e}")),
    }
}

async fn handle_capture_manifest(
    ctx: &SemRegToolContext<'_>,
    args: &serde_json::Value,
) -> SemRegToolResult {
    let session_id = match parse_uuid_arg(args, "session_id") {
        Ok(id) => id,
        Err(e) => return SemRegToolResult::err(e),
    };

    let assume_principal = args
        .get("assume_principal")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Load current focus
    let focus = match FocusStore::get(&ctx.pool, session_id).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            return SemRegToolResult::err(format!(
                "No focus state for session {session_id}. Use stew_set_focus first."
            ))
        }
        Err(e) => return SemRegToolResult::err(format!("Failed to load focus: {e}")),
    };

    // Compute ShowPacket to get viewport models
    let packet = match ShowLoop::compute_show_packet(
        &ctx.pool,
        &focus,
        &ctx.actor.actor_id,
        assume_principal.as_deref(),
    )
    .await
    {
        Ok(p) => p,
        Err(e) => return SemRegToolResult::err(format!("Failed to compute viewports: {e}")),
    };

    // We need ViewportModels (not ViewportSpecs) for the manifest.
    // For now, create minimal models from the specs in the packet.
    let viewport_models: Vec<ViewportModel> = packet
        .viewports
        .iter()
        .map(|spec| ViewportModel {
            id: spec.id.clone(),
            kind: spec.kind.clone(),
            status: ViewportStatus::Ready,
            data: spec.params.clone(),
            meta: ViewportMeta {
                updated_at: chrono::Utc::now(),
                sources: vec![],
                overlay_mode: focus.overlay_mode.clone(),
            },
        })
        .collect();

    let manifest =
        ShowLoop::compute_manifest(&focus, &viewport_models, assume_principal.as_deref());

    // Persist to database
    if let Err(e) = ShowLoop::persist_manifest(&ctx.pool, &manifest).await {
        return SemRegToolResult::err(format!("Failed to persist manifest: {e}"));
    }

    match serde_json::to_value(&manifest) {
        Ok(v) => SemRegToolResult::ok(json!({
            "status": "manifest_captured",
            "manifest_id": manifest.manifest_id.to_string(),
            "viewport_count": manifest.rendered_viewports.len(),
            "manifest": v,
        })),
        Err(e) => SemRegToolResult::err(format!("Serialization error: {e}")),
    }
}

// ─── Helpers ─────────────────────────────────────────────────────

fn parse_uuid_arg(args: &serde_json::Value, name: &str) -> Result<Uuid, String> {
    let s = args
        .get(name)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Missing required parameter: {name}"))?;
    Uuid::parse_str(s).map_err(|e| format!("Invalid UUID for {name}: {e}"))
}

fn parse_optional_uuid_arg(args: &serde_json::Value, name: &str) -> Option<Uuid> {
    args.get(name)
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase1_tool_specs_count() {
        let specs = phase1_tool_specs();
        assert_eq!(specs.len(), 6);
    }

    #[test]
    fn test_phase1_tool_names() {
        let specs = phase1_tool_specs();
        let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"stew_get_focus"));
        assert!(names.contains(&"stew_set_focus"));
        assert!(names.contains(&"stew_show"));
        assert!(names.contains(&"stew_get_viewport"));
        assert!(names.contains(&"stew_get_diff"));
        assert!(names.contains(&"stew_capture_manifest"));
    }

    #[test]
    fn test_phase1_tools_all_visualisation_category() {
        let specs = phase1_tool_specs();
        for spec in &specs {
            assert_eq!(spec.category, "visualisation");
        }
    }

    #[test]
    fn test_parse_uuid_arg_valid() {
        let args = json!({"session_id": "550e8400-e29b-41d4-a716-446655440000"});
        let result = parse_uuid_arg(&args, "session_id");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_uuid_arg_missing() {
        let args = json!({});
        let result = parse_uuid_arg(&args, "session_id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required parameter"));
    }

    #[test]
    fn test_parse_uuid_arg_invalid() {
        let args = json!({"session_id": "not-a-uuid"});
        let result = parse_uuid_arg(&args, "session_id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid UUID"));
    }
}
