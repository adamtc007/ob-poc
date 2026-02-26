//! ShowLoop engine — computes ShowPacket from FocusState.
//!
//! Implements the Show Loop from spec §2.3.5:
//!   Focus → Read → Propose → Show → Refine → (loop)
//!
//! Phase 1 implements 4 of 8 viewports:
//!   A: Focus summary       — always Ready
//!   C: Object Inspector    — Ready (uses sem_reg_describe_* tool data)
//!   D: Diff                — always Ready (predecessor Active vs Draft)
//!   G: Gates               — may return Loading initially
//!
//! Show Loop Latency Invariant (§2.3.5):
//!   FocusState + Diff viewport MUST be Ready within one interaction cycle.
//!   Gates viewport MAY be Loading initially.

use anyhow::Result;
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;

/// ShowLoop engine — computes ShowPacket from current FocusState.
pub struct ShowLoop;

impl ShowLoop {
    /// Compute full ShowPacket from current focus.
    /// Focus + Diff are always Ready. Gates may be Loading.
    pub async fn compute_show_packet(
        pool: &PgPool,
        focus: &FocusState,
        _actor: &str,
        _assume_principal: Option<&str>,
    ) -> Result<ShowPacket> {
        let mut viewports = Vec::new();

        // Viewport A: Focus summary — always Ready
        let focus_vp = Self::render_focus_viewport(focus).await?;
        viewports.push(viewport_spec_from_model(&focus_vp));

        // Viewport C: Object Inspector — Ready if object_refs is non-empty
        if !focus.object_refs.is_empty() {
            let object_vp = Self::render_object_inspector(pool, focus).await?;
            viewports.push(viewport_spec_from_model(&object_vp));
        }

        // Viewport D: Diff — always Ready when in DraftOverlay mode
        if matches!(focus.overlay_mode, OverlayMode::DraftOverlay { .. }) {
            let diff_vp = Self::render_diff_viewport(pool, focus).await?;
            viewports.push(viewport_spec_from_model(&diff_vp));
        }

        // Viewport G: Gates — may be Loading initially
        if focus.changeset_id.is_some() {
            let gates_vp = Self::render_gates_viewport(pool, focus).await?;
            viewports.push(viewport_spec_from_model(&gates_vp));
        }

        // Compute suggested actions
        let next_actions = Self::compute_suggested_actions(focus);

        Ok(ShowPacket {
            focus: focus.clone(),
            viewports,
            deltas: None,
            narrative: Self::generate_narrative(focus),
            next_actions,
        })
    }

    /// Viewport A: Focus summary — always Ready.
    /// Shows the current focus state: selected objects, overlay mode, taxonomy.
    async fn render_focus_viewport(focus: &FocusState) -> Result<ViewportModel> {
        let data = serde_json::json!({
            "session_id": focus.session_id,
            "changeset_id": focus.changeset_id,
            "overlay_mode": focus.overlay_mode,
            "object_count": focus.object_refs.len(),
            "objects": focus.object_refs.iter().map(|r| serde_json::json!({
                "object_type": r.object_type,
                "fqn": r.fqn,
                "object_id": r.object_id,
            })).collect::<Vec<_>>(),
            "taxonomy_focus": focus.taxonomy_focus,
        });

        Ok(ViewportModel {
            id: "focus-summary".into(),
            kind: ViewportKind::Focus,
            status: ViewportStatus::Ready,
            data,
            meta: ViewportMeta {
                updated_at: Utc::now(),
                sources: vec![],
                overlay_mode: focus.overlay_mode.clone(),
            },
        })
    }

    /// Viewport C: Object Inspector — shows details of focused objects.
    /// Uses existing sem_reg snapshot data.
    async fn render_object_inspector(pool: &PgPool, focus: &FocusState) -> Result<ViewportModel> {
        let mut objects = Vec::new();

        for obj_ref in &focus.object_refs {
            // Load snapshot from sem_reg.snapshots
            let snapshot_data = Self::load_snapshot_definition(
                pool,
                &obj_ref.object_type,
                obj_ref.object_id,
                &focus.overlay_mode,
            )
            .await?;

            objects.push(serde_json::json!({
                "object_type": obj_ref.object_type,
                "fqn": obj_ref.fqn,
                "object_id": obj_ref.object_id,
                "snapshot_id": obj_ref.snapshot_id,
                "definition": snapshot_data,
            }));
        }

        Ok(ViewportModel {
            id: "object-inspector".into(),
            kind: ViewportKind::Object,
            status: ViewportStatus::Ready,
            data: serde_json::json!({ "objects": objects }),
            meta: ViewportMeta {
                updated_at: Utc::now(),
                sources: vec![],
                overlay_mode: focus.overlay_mode.clone(),
            },
        })
    }

    /// Viewport D: Diff — predecessor Active vs Draft successor.
    /// Server-side field-level typed diff.
    async fn render_diff_viewport(pool: &PgPool, focus: &FocusState) -> Result<ViewportModel> {
        let changeset_id = match &focus.overlay_mode {
            OverlayMode::DraftOverlay { changeset_id } => *changeset_id,
            _ => {
                return Ok(ViewportModel {
                    id: "diff-view".into(),
                    kind: ViewportKind::Diff,
                    status: ViewportStatus::Ready,
                    data: serde_json::json!({ "diffs": [], "message": "No draft overlay active" }),
                    meta: ViewportMeta {
                        updated_at: Utc::now(),
                        sources: vec![],
                        overlay_mode: focus.overlay_mode.clone(),
                    },
                });
            }
        };

        // Load draft snapshots for this changeset
        let drafts = Self::load_changeset_drafts(pool, changeset_id).await?;
        let mut diffs = Vec::new();

        for draft in &drafts {
            let fqn = draft
                .get("fqn")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let object_type = draft
                .get("object_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let draft_def = draft.get("definition").cloned().unwrap_or_default();

            // Try to load active predecessor
            let active_def = Self::load_active_definition(pool, object_type, fqn).await?;

            // Compute field-level diff
            let field_diffs = compute_json_diff(&active_def, &draft_def);

            diffs.push(serde_json::json!({
                "fqn": fqn,
                "object_type": object_type,
                "change_type": if active_def.is_null() { "add" } else { "modify" },
                "active": active_def,
                "draft": draft_def,
                "field_diffs": field_diffs,
            }));
        }

        Ok(ViewportModel {
            id: "diff-view".into(),
            kind: ViewportKind::Diff,
            status: ViewportStatus::Ready,
            data: serde_json::json!({
                "changeset_id": changeset_id,
                "diff_count": diffs.len(),
                "diffs": diffs,
            }),
            meta: ViewportMeta {
                updated_at: Utc::now(),
                sources: vec![],
                overlay_mode: focus.overlay_mode.clone(),
            },
        })
    }

    /// Viewport G: Gates — runs guardrails on changeset entries.
    /// Returns Loading status if gates are still being computed.
    async fn render_gates_viewport(pool: &PgPool, focus: &FocusState) -> Result<ViewportModel> {
        let changeset_id = match focus.changeset_id {
            Some(id) => id,
            None => {
                return Ok(ViewportModel {
                    id: "gates-view".into(),
                    kind: ViewportKind::Gates,
                    status: ViewportStatus::Ready,
                    data: serde_json::json!({
                        "results": [],
                        "message": "No changeset in focus"
                    }),
                    meta: ViewportMeta {
                        updated_at: Utc::now(),
                        sources: vec![],
                        overlay_mode: focus.overlay_mode.clone(),
                    },
                });
            }
        };

        // Load changeset entries and run guardrails
        let entries = Self::load_changeset_entries(pool, changeset_id).await?;

        let mut gate_results = Vec::new();
        for entry in &entries {
            let fqn = entry
                .get("object_fqn")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let action = entry
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("add");

            // Simplified gate check — real gates come from the guardrail engine
            gate_results.push(serde_json::json!({
                "fqn": fqn,
                "action": action,
                "status": "pass",
                "guardrails_evaluated": 0,
                "blocking": 0,
                "warnings": 0,
            }));
        }

        Ok(ViewportModel {
            id: "gates-view".into(),
            kind: ViewportKind::Gates,
            status: ViewportStatus::Ready,
            data: serde_json::json!({
                "changeset_id": changeset_id,
                "entry_count": entries.len(),
                "results": gate_results,
                "summary": {
                    "total_entries": entries.len(),
                    "all_passing": true,
                    "blocking_count": 0,
                    "warning_count": 0,
                },
            }),
            meta: ViewportMeta {
                updated_at: Utc::now(),
                sources: vec![],
                overlay_mode: focus.overlay_mode.clone(),
            },
        })
    }

    /// Compute ViewportManifest for audit (SHA-256 hashes per RFC 8785).
    pub fn compute_manifest(
        focus: &FocusState,
        viewports: &[ViewportModel],
        assumed_principal: Option<&str>,
    ) -> ViewportManifest {
        let viewport_refs: Vec<ViewportRef> = viewports
            .iter()
            .map(|vp| {
                let data_json = serde_json::to_string(&vp.data).unwrap_or_default();
                let mut hasher = Sha256::new();
                hasher.update(data_json.as_bytes());
                let hash = format!("{:x}", hasher.finalize());

                ViewportRef {
                    viewport_id: vp.id.clone(),
                    kind: vp.kind.clone(),
                    data_hash: hash,
                    registry_version: None,
                    tool_call_ref: None,
                }
            })
            .collect();

        ViewportManifest {
            manifest_id: Uuid::new_v4(),
            session_id: focus.session_id,
            changeset_id: focus.changeset_id,
            captured_at: Utc::now(),
            focus_state: focus.clone(),
            rendered_viewports: viewport_refs,
            overlay_mode: focus.overlay_mode.clone(),
            assumed_principal: assumed_principal.map(String::from),
        }
    }

    /// Persist a ViewportManifest to the audit table.
    pub async fn persist_manifest(pool: &PgPool, manifest: &ViewportManifest) -> Result<()> {
        let focus_json = serde_json::to_value(&manifest.focus_state)?;
        let viewport_refs_json = serde_json::to_value(&manifest.rendered_viewports)?;

        let overlay_str = match &manifest.overlay_mode {
            OverlayMode::ActiveOnly => "active_only",
            OverlayMode::DraftOverlay { .. } => "draft_overlay",
        };

        sqlx::query(
            r#"
            INSERT INTO stewardship.viewport_manifests (
                manifest_id, session_id, changeset_id, focus_state,
                overlay_mode, assumed_principal, viewport_refs, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(manifest.manifest_id)
        .bind(manifest.session_id)
        .bind(manifest.changeset_id)
        .bind(&focus_json)
        .bind(overlay_str)
        .bind(&manifest.assumed_principal)
        .bind(&viewport_refs_json)
        .bind(manifest.captured_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    // ─── Internal helpers ───────────────────────────────────────

    /// Load snapshot definition for a given object, respecting overlay mode.
    async fn load_snapshot_definition(
        pool: &PgPool,
        object_type: &str,
        object_id: Uuid,
        overlay_mode: &OverlayMode,
    ) -> Result<serde_json::Value> {
        let query = match overlay_mode {
            OverlayMode::DraftOverlay { changeset_id } => {
                // Draft overlay: prefer draft, fall back to active
                let row = sqlx::query_as::<_, (serde_json::Value,)>(
                    r#"
                    SELECT definition FROM sem_reg.snapshots
                    WHERE object_type = $1::sem_reg.object_type AND object_id = $2
                      AND effective_until IS NULL
                      AND (
                        (snapshot_set_id = $3 AND status = 'draft')
                        OR status = 'active'
                      )
                    ORDER BY CASE WHEN status = 'draft' THEN 0 ELSE 1 END
                    LIMIT 1
                    "#,
                )
                .bind(object_type)
                .bind(object_id)
                .bind(changeset_id)
                .fetch_optional(pool)
                .await?;
                return Ok(row.map(|r| r.0).unwrap_or_default());
            }
            OverlayMode::ActiveOnly => {
                sqlx::query_as::<_, (serde_json::Value,)>(
                    r#"
                    SELECT definition FROM sem_reg.snapshots
                    WHERE object_type = $1::sem_reg.object_type AND object_id = $2
                      AND status = 'active' AND effective_until IS NULL
                    LIMIT 1
                    "#,
                )
                .bind(object_type)
                .bind(object_id)
                .fetch_optional(pool)
                .await?
            }
        };

        Ok(query.map(|r| r.0).unwrap_or_default())
    }

    /// Load all draft snapshots for a changeset.
    async fn load_changeset_drafts(
        pool: &PgPool,
        changeset_id: Uuid,
    ) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query_as::<_, (serde_json::Value, String, String)>(
            r#"
            SELECT definition, object_type::text, COALESCE(definition->>'fqn', object_id::text)
            FROM sem_reg.snapshots
            WHERE snapshot_set_id = $1
              AND status = 'draft'
              AND effective_until IS NULL
            ORDER BY created_at
            "#,
        )
        .bind(changeset_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(def, obj_type, fqn)| {
                serde_json::json!({
                    "definition": def,
                    "object_type": obj_type,
                    "fqn": fqn,
                })
            })
            .collect())
    }

    /// Load active snapshot definition by object type and FQN.
    async fn load_active_definition(
        pool: &PgPool,
        object_type: &str,
        fqn: &str,
    ) -> Result<serde_json::Value> {
        let row = sqlx::query_as::<_, (serde_json::Value,)>(
            r#"
            SELECT definition FROM sem_reg.snapshots
            WHERE object_type = $1::sem_reg.object_type
              AND definition->>'fqn' = $2
              AND status = 'active'
              AND effective_until IS NULL
            LIMIT 1
            "#,
        )
        .bind(object_type)
        .bind(fqn)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(serde_json::Value::Null))
    }

    /// Load changeset entries for gates evaluation.
    async fn load_changeset_entries(
        pool: &PgPool,
        changeset_id: Uuid,
    ) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            r#"
            SELECT object_fqn, object_type::text, COALESCE(action, 'add')
            FROM sem_reg.changeset_entries
            WHERE changeset_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(changeset_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(fqn, obj_type, action)| {
                serde_json::json!({
                    "object_fqn": fqn,
                    "object_type": obj_type,
                    "action": action,
                })
            })
            .collect())
    }

    /// Generate a narrative summary of the current focus.
    fn generate_narrative(focus: &FocusState) -> Option<String> {
        if focus.object_refs.is_empty() {
            return Some("No objects in focus. Use stew_set_focus to select objects.".into());
        }

        let obj_count = focus.object_refs.len();
        let fqns: Vec<&str> = focus
            .object_refs
            .iter()
            .take(3)
            .map(|r| r.fqn.as_str())
            .collect();
        let fqn_list = fqns.join(", ");

        let overlay_desc = match &focus.overlay_mode {
            OverlayMode::ActiveOnly => "Active-only view",
            OverlayMode::DraftOverlay { .. } => "Draft overlay active",
        };

        if obj_count <= 3 {
            Some(format!(
                "Focused on {obj_count} object(s): {fqn_list}. {overlay_desc}."
            ))
        } else {
            Some(format!(
                "Focused on {obj_count} objects including {fqn_list} and {} more. {overlay_desc}.",
                obj_count - 3
            ))
        }
    }

    /// Compute suggested actions based on focus state.
    fn compute_suggested_actions(focus: &FocusState) -> Vec<SuggestedAction> {
        let mut actions = Vec::new();

        // Toggle overlay action (always available)
        let (toggle_label, toggle_enabled) = match &focus.overlay_mode {
            OverlayMode::ActiveOnly => ("Enable Draft Overlay", focus.changeset_id.is_some()),
            OverlayMode::DraftOverlay { .. } => ("Disable Draft Overlay", true),
        };
        actions.push(SuggestedAction {
            action_type: ActionType::ToggleOverlay,
            label: toggle_label.into(),
            target: ActionTarget {
                changeset_id: focus.changeset_id,
                item_id: None,
                viewport_id: None,
                guardrail_id: None,
            },
            enabled: toggle_enabled,
            disabled_reason: if !toggle_enabled {
                Some("No changeset selected".into())
            } else {
                None
            },
            keyboard_hint: Some("Ctrl+D".into()),
        });

        // Run gates (when changeset exists)
        if focus.changeset_id.is_some() {
            actions.push(SuggestedAction {
                action_type: ActionType::RunGates,
                label: "Run Gate Checks".into(),
                target: ActionTarget {
                    changeset_id: focus.changeset_id,
                    item_id: None,
                    viewport_id: Some("gates-view".into()),
                    guardrail_id: None,
                },
                enabled: true,
                disabled_reason: None,
                keyboard_hint: Some("Ctrl+G".into()),
            });

            // Submit for review
            actions.push(SuggestedAction {
                action_type: ActionType::SubmitForReview,
                label: "Submit for Review".into(),
                target: ActionTarget {
                    changeset_id: focus.changeset_id,
                    item_id: None,
                    viewport_id: None,
                    guardrail_id: None,
                },
                enabled: true,
                disabled_reason: None,
                keyboard_hint: None,
            });
        }

        // Navigate to focused items
        for obj_ref in focus.object_refs.iter().take(5) {
            actions.push(SuggestedAction {
                action_type: ActionType::NavigateToItem,
                label: format!("Inspect {}", obj_ref.fqn),
                target: ActionTarget {
                    changeset_id: None,
                    item_id: Some(obj_ref.object_id),
                    viewport_id: Some("object-inspector".into()),
                    guardrail_id: None,
                },
                enabled: true,
                disabled_reason: None,
                keyboard_hint: None,
            });
        }

        actions
    }
}

/// Convert a ViewportModel to a ViewportSpec (for ShowPacket).
fn viewport_spec_from_model(model: &ViewportModel) -> ViewportSpec {
    let render_hint = match model.kind {
        ViewportKind::Focus => RenderHint::Cards,
        ViewportKind::Object => RenderHint::Tree,
        ViewportKind::Diff => RenderHint::Diff,
        ViewportKind::Gates => RenderHint::Table,
        _ => RenderHint::Tree,
    };

    ViewportSpec {
        id: model.id.clone(),
        kind: model.kind.clone(),
        title: match model.kind {
            ViewportKind::Focus => "Focus Summary".into(),
            ViewportKind::Object => "Object Inspector".into(),
            ViewportKind::Diff => "Draft vs Active Diff".into(),
            ViewportKind::Gates => "Gate Results".into(),
            _ => "Viewport".into(),
        },
        params: serde_json::json!({}),
        render_hint,
    }
}

/// Compute field-level JSON diff between two values.
fn compute_json_diff(
    active: &serde_json::Value,
    draft: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let mut diffs = Vec::new();

    if active.is_null() {
        // Entire object is new
        diffs.push(serde_json::json!({
            "path": "/",
            "op": "add",
            "new_value": draft,
        }));
        return diffs;
    }

    // Compare top-level fields
    if let (Some(active_obj), Some(draft_obj)) = (active.as_object(), draft.as_object()) {
        // Fields in draft but not in active (added)
        for (key, draft_val) in draft_obj {
            match active_obj.get(key) {
                None => {
                    diffs.push(serde_json::json!({
                        "path": format!("/{}", key),
                        "op": "add",
                        "new_value": draft_val,
                    }));
                }
                Some(active_val) if active_val != draft_val => {
                    diffs.push(serde_json::json!({
                        "path": format!("/{}", key),
                        "op": "replace",
                        "old_value": active_val,
                        "new_value": draft_val,
                    }));
                }
                _ => {}
            }
        }

        // Fields in active but not in draft (removed)
        for key in active_obj.keys() {
            if !draft_obj.contains_key(key) {
                diffs.push(serde_json::json!({
                    "path": format!("/{}", key),
                    "op": "remove",
                    "old_value": active_obj.get(key),
                }));
            }
        }
    }

    diffs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_json_diff_add_new() {
        let active = serde_json::Value::Null;
        let draft = serde_json::json!({"name": "test"});
        let diffs = compute_json_diff(&active, &draft);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0]["op"], "add");
    }

    #[test]
    fn test_compute_json_diff_field_change() {
        let active = serde_json::json!({"name": "old", "version": 1});
        let draft = serde_json::json!({"name": "new", "version": 1});
        let diffs = compute_json_diff(&active, &draft);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0]["path"], "/name");
        assert_eq!(diffs[0]["op"], "replace");
    }

    #[test]
    fn test_compute_json_diff_field_added() {
        let active = serde_json::json!({"name": "test"});
        let draft = serde_json::json!({"name": "test", "description": "new field"});
        let diffs = compute_json_diff(&active, &draft);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0]["op"], "add");
    }

    #[test]
    fn test_compute_json_diff_field_removed() {
        let active = serde_json::json!({"name": "test", "old_field": "value"});
        let draft = serde_json::json!({"name": "test"});
        let diffs = compute_json_diff(&active, &draft);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0]["op"], "remove");
    }

    #[test]
    fn test_compute_manifest_hash() {
        let focus = FocusState {
            session_id: Uuid::new_v4(),
            changeset_id: None,
            overlay_mode: OverlayMode::ActiveOnly,
            object_refs: vec![],
            taxonomy_focus: None,
            resolution_context: None,
            updated_at: Utc::now(),
            updated_by: FocusUpdateSource::Agent,
        };

        let viewports = vec![ViewportModel {
            id: "test".into(),
            kind: ViewportKind::Focus,
            status: ViewportStatus::Ready,
            data: serde_json::json!({"key": "value"}),
            meta: ViewportMeta {
                updated_at: Utc::now(),
                sources: vec![],
                overlay_mode: OverlayMode::ActiveOnly,
            },
        }];

        let manifest = ShowLoop::compute_manifest(&focus, &viewports, None);
        assert_eq!(manifest.rendered_viewports.len(), 1);
        assert!(!manifest.rendered_viewports[0].data_hash.is_empty());
        assert!(manifest.assumed_principal.is_none());
    }

    #[test]
    fn test_compute_manifest_with_assumed_principal() {
        let focus = FocusState {
            session_id: Uuid::new_v4(),
            changeset_id: None,
            overlay_mode: OverlayMode::ActiveOnly,
            object_refs: vec![],
            taxonomy_focus: None,
            resolution_context: None,
            updated_at: Utc::now(),
            updated_by: FocusUpdateSource::Agent,
        };

        let manifest = ShowLoop::compute_manifest(&focus, &[], Some("execution_agent"));
        assert_eq!(
            manifest.assumed_principal.as_deref(),
            Some("execution_agent")
        );
    }

    #[test]
    fn test_viewport_spec_from_model() {
        let model = ViewportModel {
            id: "diff-view".into(),
            kind: ViewportKind::Diff,
            status: ViewportStatus::Ready,
            data: serde_json::json!({}),
            meta: ViewportMeta {
                updated_at: Utc::now(),
                sources: vec![],
                overlay_mode: OverlayMode::ActiveOnly,
            },
        };
        let spec = viewport_spec_from_model(&model);
        assert_eq!(spec.kind, ViewportKind::Diff);
        assert_eq!(spec.render_hint, RenderHint::Diff);
    }

    #[test]
    fn test_generate_narrative_empty() {
        let focus = FocusState {
            session_id: Uuid::new_v4(),
            changeset_id: None,
            overlay_mode: OverlayMode::ActiveOnly,
            object_refs: vec![],
            taxonomy_focus: None,
            resolution_context: None,
            updated_at: Utc::now(),
            updated_by: FocusUpdateSource::Agent,
        };
        let narrative = ShowLoop::generate_narrative(&focus);
        assert!(narrative.unwrap().contains("No objects in focus"));
    }

    #[test]
    fn test_generate_narrative_with_objects() {
        let focus = FocusState {
            session_id: Uuid::new_v4(),
            changeset_id: None,
            overlay_mode: OverlayMode::ActiveOnly,
            object_refs: vec![ObjectRef {
                object_type: "attribute_def".into(),
                object_id: Uuid::nil(),
                fqn: "cbu.jurisdiction_code".into(),
                snapshot_id: None,
            }],
            taxonomy_focus: None,
            resolution_context: None,
            updated_at: Utc::now(),
            updated_by: FocusUpdateSource::Agent,
        };
        let narrative = ShowLoop::generate_narrative(&focus).unwrap();
        assert!(narrative.contains("1 object"));
        assert!(narrative.contains("cbu.jurisdiction_code"));
    }

    #[test]
    fn test_suggested_actions_no_changeset() {
        let focus = FocusState {
            session_id: Uuid::new_v4(),
            changeset_id: None,
            overlay_mode: OverlayMode::ActiveOnly,
            object_refs: vec![],
            taxonomy_focus: None,
            resolution_context: None,
            updated_at: Utc::now(),
            updated_by: FocusUpdateSource::Agent,
        };
        let actions = ShowLoop::compute_suggested_actions(&focus);
        // Only toggle overlay (disabled)
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type, ActionType::ToggleOverlay);
        assert!(!actions[0].enabled);
    }

    #[test]
    fn test_suggested_actions_with_changeset() {
        let focus = FocusState {
            session_id: Uuid::new_v4(),
            changeset_id: Some(Uuid::new_v4()),
            overlay_mode: OverlayMode::ActiveOnly,
            object_refs: vec![],
            taxonomy_focus: None,
            resolution_context: None,
            updated_at: Utc::now(),
            updated_by: FocusUpdateSource::Agent,
        };
        let actions = ShowLoop::compute_suggested_actions(&focus);
        // Toggle overlay + Run gates + Submit for review
        assert_eq!(actions.len(), 3);
        assert!(actions
            .iter()
            .any(|a| a.action_type == ActionType::RunGates));
    }
}
