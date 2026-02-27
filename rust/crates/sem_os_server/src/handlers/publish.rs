//! POST /publish — admin-only publish endpoint.
//!
//! Accepts a seed bundle and publishes it through the core service.

use std::sync::Arc;

use axum::{extract::Extension, Json};
use sem_os_core::{
    principal::Principal, proto::BootstrapSeedBundleResponse, seeds::SeedBundle,
    service::CoreService,
};

use crate::error::AppError;

pub async fn publish(
    Extension(principal): Extension<Principal>,
    Extension(service): Extension<Arc<dyn CoreService>>,
    Json(bundle): Json<SeedBundle>,
) -> Result<Json<BootstrapSeedBundleResponse>, AppError> {
    principal.require_admin()?;
    let resp = service.bootstrap_seed_bundle(&principal, bundle).await?;
    Ok(Json(resp))
}
