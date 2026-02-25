//! Router construction for the Semantic OS server.

use std::sync::Arc;

use axum::{
    middleware as axum_mw,
    routing::{get, post},
    Extension, Router,
};
use sem_os_core::service::CoreService;
use sqlx::PgPool;

use crate::handlers;
use crate::middleware::jwt::{jwt_auth, JwtConfig};

/// Build the full axum router with all routes and middleware.
pub fn build_router(service: Arc<dyn CoreService>, pool: PgPool, jwt_config: JwtConfig) -> Router {
    // Routes that require JWT authentication
    let protected = Router::new()
        .route(
            "/resolve_context",
            post(handlers::resolve_context::resolve_context),
        )
        .route(
            "/snapshot_sets/{id}/manifest",
            get(handlers::manifest::get_manifest),
        )
        .route("/publish", post(handlers::publish::publish))
        .route(
            "/exports/snapshot_set/{id}",
            get(handlers::export::export_snapshot_set),
        )
        .route(
            "/bootstrap/seed_bundle",
            post(handlers::bootstrap::bootstrap_seed_bundle),
        )
        .route("/tools/call", post(handlers::tools::call_tool))
        .route("/tools/list", get(handlers::tools::list_tools))
        // Changeset / Workbench
        .route("/changesets", get(handlers::changesets::list_changesets))
        .route(
            "/changesets/{id}/diff",
            get(handlers::changesets::changeset_diff),
        )
        .route(
            "/changesets/{id}/impact",
            get(handlers::changesets::changeset_impact),
        )
        .route(
            "/changesets/{id}/gate_preview",
            post(handlers::changesets::changeset_gate_preview),
        )
        .route(
            "/changesets/{id}/publish",
            post(handlers::changesets::publish_changeset),
        )
        .layer(axum_mw::from_fn(jwt_auth))
        .layer(Extension(jwt_config));

    // Public routes (no auth)
    let public = Router::new().route("/health", get(handlers::health::health));

    // Combine and add shared state
    public
        .merge(protected)
        .layer(Extension(service))
        .layer(Extension(pool))
}
