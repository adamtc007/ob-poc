//! FocusState store — CRUD + FocusChanged audit event emission.
//!
//! Server-side shared truth (spec §9.14.1): same record updated by
//! agent and UI. Every mutation emits a FocusChanged stewardship event
//! for the audit chain.

use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::store::StewardshipStore;
use super::types::*;

/// FocusState store operations.
pub struct FocusStore;

impl FocusStore {
    /// Get the current focus state for a session.
    pub async fn get(pool: &PgPool, session_id: Uuid) -> Result<Option<FocusState>> {
        let row = sqlx::query_as::<_, FocusRow>(
            r#"
            SELECT session_id, changeset_id, overlay_mode, overlay_changeset_id,
                   object_refs, taxonomy_focus, resolution_context,
                   updated_at, updated_by
            FROM stewardship.focus_states
            WHERE session_id = $1
            "#,
        )
        .bind(session_id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(Some(r.into_focus_state()?)),
            None => Ok(None),
        }
    }

    /// Set focus state — upserts and emits FocusChanged audit event.
    pub async fn set(
        pool: &PgPool,
        focus: &FocusState,
        source: FocusUpdateSource,
        changeset_id: Option<Uuid>,
    ) -> Result<()> {
        let object_refs_json = serde_json::to_value(&focus.object_refs)?;
        let taxonomy_focus_json = focus
            .taxonomy_focus
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;

        let (overlay_mode_str, overlay_changeset_id) = match &focus.overlay_mode {
            OverlayMode::ActiveOnly => ("active_only", None),
            OverlayMode::DraftOverlay { changeset_id } => {
                ("draft_overlay", Some(*changeset_id))
            }
        };

        sqlx::query(
            r#"
            INSERT INTO stewardship.focus_states (
                session_id, changeset_id, overlay_mode, overlay_changeset_id,
                object_refs, taxonomy_focus, resolution_context,
                updated_at, updated_by
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (session_id) DO UPDATE SET
                changeset_id = EXCLUDED.changeset_id,
                overlay_mode = EXCLUDED.overlay_mode,
                overlay_changeset_id = EXCLUDED.overlay_changeset_id,
                object_refs = EXCLUDED.object_refs,
                taxonomy_focus = EXCLUDED.taxonomy_focus,
                resolution_context = EXCLUDED.resolution_context,
                updated_at = EXCLUDED.updated_at,
                updated_by = EXCLUDED.updated_by
            "#,
        )
        .bind(focus.session_id)
        .bind(focus.changeset_id)
        .bind(overlay_mode_str)
        .bind(overlay_changeset_id)
        .bind(&object_refs_json)
        .bind(&taxonomy_focus_json)
        .bind(&focus.resolution_context)
        .bind(Utc::now())
        .bind(source.as_str())
        .execute(pool)
        .await?;

        // Emit FocusChanged audit event
        let event = StewardshipRecord {
            event_id: Uuid::new_v4(),
            changeset_id: changeset_id.or(focus.changeset_id).unwrap_or(Uuid::nil()),
            event_type: StewardshipEventType::FocusChanged {
                from: serde_json::json!({}),
                to: serde_json::to_value(focus).unwrap_or_default(),
                source: source.clone(),
            },
            actor_id: source.as_str().to_string(),
            payload: serde_json::json!({
                "session_id": focus.session_id,
            }),
            viewport_manifest_id: None,
            created_at: Utc::now(),
        };
        StewardshipStore::append_event(pool, &event).await?;

        Ok(())
    }

    /// Delete focus state for a session.
    pub async fn delete(pool: &PgPool, session_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM stewardship.focus_states WHERE session_id = $1")
            .bind(session_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

// ─── Internal row types ─────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct FocusRow {
    session_id: Uuid,
    changeset_id: Option<Uuid>,
    overlay_mode: String,
    overlay_changeset_id: Option<Uuid>,
    object_refs: serde_json::Value,
    taxonomy_focus: Option<serde_json::Value>,
    resolution_context: Option<serde_json::Value>,
    updated_at: chrono::DateTime<chrono::Utc>,
    updated_by: String,
}

impl FocusRow {
    fn into_focus_state(self) -> Result<FocusState> {
        let overlay_mode = match self.overlay_mode.as_str() {
            "draft_overlay" => {
                let cs_id = self
                    .overlay_changeset_id
                    .ok_or_else(|| anyhow::anyhow!("draft_overlay requires overlay_changeset_id"))?;
                OverlayMode::DraftOverlay {
                    changeset_id: cs_id,
                }
            }
            _ => OverlayMode::ActiveOnly,
        };

        let object_refs: Vec<ObjectRef> = serde_json::from_value(self.object_refs)?;
        let taxonomy_focus: Option<TaxonomyFocus> = self
            .taxonomy_focus
            .map(serde_json::from_value)
            .transpose()?;
        let updated_by = match self.updated_by.as_str() {
            "user_navigation" => FocusUpdateSource::UserNavigation,
            _ => FocusUpdateSource::Agent,
        };

        Ok(FocusState {
            session_id: self.session_id,
            changeset_id: self.changeset_id,
            overlay_mode,
            object_refs,
            taxonomy_focus,
            resolution_context: self.resolution_context,
            updated_at: self.updated_at,
            updated_by,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_row_active_only() {
        let row = FocusRow {
            session_id: Uuid::new_v4(),
            changeset_id: None,
            overlay_mode: "active_only".into(),
            overlay_changeset_id: None,
            object_refs: serde_json::json!([]),
            taxonomy_focus: None,
            resolution_context: None,
            updated_at: Utc::now(),
            updated_by: "agent".into(),
        };
        let focus = row.into_focus_state().unwrap();
        assert_eq!(focus.overlay_mode, OverlayMode::ActiveOnly);
    }

    #[test]
    fn test_focus_row_draft_overlay() {
        let cs_id = Uuid::new_v4();
        let row = FocusRow {
            session_id: Uuid::new_v4(),
            changeset_id: Some(cs_id),
            overlay_mode: "draft_overlay".into(),
            overlay_changeset_id: Some(cs_id),
            object_refs: serde_json::json!([{
                "object_type": "attribute_def",
                "object_id": Uuid::nil().to_string(),
                "fqn": "cbu.jurisdiction_code",
                "snapshot_id": null
            }]),
            taxonomy_focus: None,
            resolution_context: None,
            updated_at: Utc::now(),
            updated_by: "user_navigation".into(),
        };
        let focus = row.into_focus_state().unwrap();
        assert_eq!(
            focus.overlay_mode,
            OverlayMode::DraftOverlay {
                changeset_id: cs_id
            }
        );
        assert_eq!(focus.object_refs.len(), 1);
    }
}
