//! POST /bootstrap/seed_bundle — admin-only idempotent bootstrap endpoint.
//!
//! Checks bootstrap audit for an existing `bundle_hash` via CoreService.
//! - If `status = 'published'`: return 200 + existing `snapshot_set_id`.
//! - If `status = 'in_progress'`: return 409.
//! - Otherwise: insert `status='in_progress'` row, call core publish,
//!   update to `status='published'` + `snapshot_set_id`, return response.

use std::sync::Arc;

use axum::{extract::Extension, http::StatusCode, Json};
use sem_os_core::{
    principal::Principal, proto::BootstrapSeedBundleResponse, seeds::SeedBundle,
    service::CoreService,
};
use serde::Serialize;

use crate::error::AppError;

/// Response for the bootstrap endpoint — covers both fresh and idempotent-hit cases.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status")]
pub enum BootstrapResponse {
    #[serde(rename = "already_published")]
    AlreadyPublished {
        bundle_hash: String,
        snapshot_set_id: Option<uuid::Uuid>,
    },
    #[serde(rename = "published")]
    Published(BootstrapSeedBundleResponse),
}

pub async fn bootstrap_seed_bundle(
    Extension(principal): Extension<Principal>,
    Extension(service): Extension<Arc<dyn CoreService>>,
    Json(bundle): Json<SeedBundle>,
) -> Result<(StatusCode, Json<BootstrapResponse>), AppError> {
    principal.require_admin()?;

    let bundle_hash = bundle.bundle_hash.clone();

    // Check for existing audit record
    let existing = service.bootstrap_check(&bundle_hash).await?;

    if let Some((status, snapshot_set_id)) = existing {
        match status.as_str() {
            "published" => {
                return Ok((
                    StatusCode::OK,
                    Json(BootstrapResponse::AlreadyPublished {
                        bundle_hash,
                        snapshot_set_id,
                    }),
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

    service
        .bootstrap_start(&bundle_hash, &principal.actor_id, bundle_counts)
        .await?;

    // Call core service
    match service.bootstrap_seed_bundle(&principal, bundle).await {
        Ok(resp) => {
            // Mark published
            service.bootstrap_mark_published(&bundle_hash).await?;
            Ok((StatusCode::OK, Json(BootstrapResponse::Published(resp))))
        }
        Err(e) => {
            // Mark failed (best-effort — don't mask original error)
            let _ = service
                .bootstrap_mark_failed(&bundle_hash, &e.to_string())
                .await;
            Err(e.into())
        }
    }
}
