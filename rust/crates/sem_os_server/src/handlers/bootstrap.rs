//! POST /bootstrap/seed_bundle — admin-only idempotent bootstrap endpoint.
//!
//! Checks `sem_reg.bootstrap_audit` for an existing `bundle_hash`.
//! - If `status = 'published'`: return 200 + existing `snapshot_set_id`.
//! - If `status = 'in_progress'`: return 409.
//! - Otherwise: insert `status='in_progress'` row, call core publish,
//!   update to `status='published'` + `snapshot_set_id`, return response.

use std::sync::Arc;

use axum::{extract::Extension, http::StatusCode, Json};
use sem_os_core::{principal::Principal, seeds::SeedBundle, service::CoreService};
use sqlx::PgPool;

use crate::error::AppError;

pub async fn bootstrap_seed_bundle(
    Extension(principal): Extension<Principal>,
    Extension(service): Extension<Arc<dyn CoreService>>,
    Extension(pool): Extension<PgPool>,
    Json(bundle): Json<SeedBundle>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    principal.require_admin()?;

    let bundle_hash = bundle.bundle_hash.clone();

    // Check for existing audit record
    let existing = sqlx::query_as::<_, (String, Option<uuid::Uuid>)>(
        r#"
        SELECT status, snapshot_set_id
        FROM sem_reg.bootstrap_audit
        WHERE bundle_hash = $1
        "#,
    )
    .bind(&bundle_hash)
    .fetch_optional(&pool)
    .await
    .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;

    if let Some((status, snapshot_set_id)) = existing {
        match status.as_str() {
            "published" => {
                return Ok((
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "status": "already_published",
                        "bundle_hash": bundle_hash,
                        "snapshot_set_id": snapshot_set_id,
                    })),
                ));
            }
            "in_progress" => {
                return Err(sem_os_core::error::SemOsError::Conflict(
                    "bootstrap already in progress for this bundle_hash".into(),
                )
                .into());
            }
            _ => {} // failed — retry
        }
    }

    // Insert in_progress audit record (or update failed→in_progress)
    let bundle_counts = serde_json::json!({
        "verb_contracts": bundle.verb_contracts.len(),
        "attributes": bundle.attributes.len(),
        "entity_types": bundle.entity_types.len(),
        "taxonomies": bundle.taxonomies.len(),
        "policies": bundle.policies.len(),
        "views": bundle.views.len(),
    });

    sqlx::query(
        r#"
        INSERT INTO sem_reg.bootstrap_audit (
            bundle_hash, origin_actor_id, bundle_counts, status
        ) VALUES ($1, $2, $3, 'in_progress')
        ON CONFLICT (bundle_hash) DO UPDATE
        SET status = 'in_progress',
            started_at = now(),
            error = NULL
        "#,
    )
    .bind(&bundle_hash)
    .bind(&principal.actor_id)
    .bind(&bundle_counts)
    .execute(&pool)
    .await
    .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;

    // Call core service
    match service.bootstrap_seed_bundle(&principal, bundle).await {
        Ok(resp) => {
            // Mark published
            sqlx::query(
                r#"
                UPDATE sem_reg.bootstrap_audit
                SET status = 'published',
                    completed_at = now()
                WHERE bundle_hash = $1
                "#,
            )
            .bind(&bundle_hash)
            .execute(&pool)
            .await
            .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;

            let json = serde_json::to_value(&resp)
                .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
            Ok((StatusCode::OK, Json(json)))
        }
        Err(e) => {
            // Mark failed
            let _ = sqlx::query(
                r#"
                UPDATE sem_reg.bootstrap_audit
                SET status = 'failed',
                    completed_at = now(),
                    error = $2
                WHERE bundle_hash = $1
                "#,
            )
            .bind(&bundle_hash)
            .bind(e.to_string())
            .execute(&pool)
            .await;

            Err(e.into())
        }
    }
}
