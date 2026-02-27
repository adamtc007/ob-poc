//! Router construction for the Semantic OS server.

use std::sync::Arc;

use axum::{
    middleware as axum_mw,
    routing::{get, post},
    Extension, Router,
};
use sem_os_core::service::CoreService;

use crate::handlers;
use crate::middleware::jwt::{jwt_auth, JwtConfig};

/// Build the full axum router with all routes and middleware.
pub fn build_router(service: Arc<dyn CoreService>, jwt_config: JwtConfig) -> Router {
    // Routes that require JWT authentication
    let protected = Router::new()
        .route(
            "/resolve_context",
            post(handlers::resolve_context::resolve_context),
        )
        .route(
            "/snapshot_sets/:id/manifest",
            get(handlers::manifest::get_manifest),
        )
        .route("/publish", post(handlers::publish::publish))
        .route(
            "/exports/snapshot_set/:id",
            get(handlers::export::export_snapshot_set),
        )
        .route(
            "/bootstrap/seed_bundle",
            post(handlers::bootstrap::bootstrap_seed_bundle),
        )
        // TODO: Re-add /tools/* routes when tool schemas are finalized
        // .route("/tools/call", post(handlers::tools::call_tool))
        // .route("/tools/list", get(handlers::tools::list_tools))
        // Authoring pipeline (governance verbs)
        .route("/authoring", get(handlers::authoring::list))
        .route("/authoring/propose", post(handlers::authoring::propose))
        .route(
            "/authoring/publish-batch",
            post(handlers::authoring::publish_batch),
        )
        .route("/authoring/diff", post(handlers::authoring::diff))
        .route("/authoring/:id", get(handlers::authoring::get))
        .route(
            "/authoring/:id/validate",
            post(handlers::authoring::validate),
        )
        .route("/authoring/:id/dry-run", post(handlers::authoring::dry_run))
        .route(
            "/authoring/:id/plan",
            get(handlers::authoring::plan_publish),
        )
        .route("/authoring/:id/publish", post(handlers::authoring::publish))
        // Changeset / Workbench
        .route("/changesets", get(handlers::changesets::list_changesets))
        .route(
            "/changesets/:id/diff",
            get(handlers::changesets::changeset_diff),
        )
        .route(
            "/changesets/:id/impact",
            get(handlers::changesets::changeset_impact),
        )
        .route(
            "/changesets/:id/gate_preview",
            post(handlers::changesets::changeset_gate_preview),
        )
        .route(
            "/changesets/:id/publish",
            post(handlers::changesets::publish_changeset),
        )
        .layer(axum_mw::from_fn(jwt_auth))
        .layer(Extension(jwt_config));

    // Public routes (no auth)
    let public = Router::new()
        .route("/health", get(handlers::health::health))
        .route(
            "/health/semreg/pending-changesets",
            get(handlers::health::semreg_pending_changesets),
        )
        .route(
            "/health/semreg/stale-dryruns",
            get(handlers::health::semreg_stale_dryruns),
        );

    // Combine and add shared state
    public.merge(protected).layer(Extension(service))
}
