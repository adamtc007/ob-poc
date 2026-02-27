//! HTTP-level integration tests for the Semantic OS authoring pipeline.
//!
//! These tests prove the deployed HTTP server contract: JWT authentication,
//! AgentMode gating, admin role enforcement, and authoring endpoint behavior.
//!
//! Requires a running PostgreSQL database with migrations applied.
//! Run with: DATABASE_URL="postgresql:///data_designer" cargo test -p sem_os_server --test authoring_http_integration -- --ignored --nocapture

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use http_body_util::BodyExt;
use hyper::{Request, StatusCode};
use jsonwebtoken::{encode, EncodingKey, Header};
use sem_os_core::ports::BootstrapAuditStore;
use sem_os_core::service::CoreServiceImpl;
use sem_os_postgres::PgStores;
use sem_os_server::middleware::jwt::JwtConfig;
use sem_os_server::router::build_router;
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

// ── Test JWT helpers ───────────────────────────────────────────

const TEST_JWT_SECRET: &[u8] = b"test-secret-for-integration-tests";

/// Claims structure for test JWT generation.
/// Matches the server's expected JwtClaims shape (sub, roles, tenancy + flattened extra).
#[derive(Debug, Serialize)]
struct TestClaims {
    sub: String,
    roles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tenancy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_mode: Option<String>,
}

fn make_jwt(actor_id: &str, roles: &[&str], agent_mode: Option<&str>) -> String {
    let claims = TestClaims {
        sub: actor_id.into(),
        roles: roles.iter().map(|r| r.to_string()).collect(),
        tenancy: None,
        agent_mode: agent_mode.map(|s| s.to_string()),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET),
    )
    .expect("failed to encode test JWT")
}

fn admin_governed_jwt() -> String {
    make_jwt("test-admin", &["admin"], Some("governed"))
}

fn viewer_governed_jwt() -> String {
    make_jwt("test-viewer", &["viewer"], Some("governed"))
}

fn admin_research_jwt() -> String {
    make_jwt("test-admin", &["admin"], Some("research"))
}

// ── Test app builder ───────────────────────────────────────────

async fn build_test_app() -> axum::Router {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to test database");

    let stores = PgStores::new(pool);

    let outbox: Arc<dyn sem_os_core::ports::OutboxStore> = Arc::new(stores.outbox);
    let projections: Arc<dyn sem_os_core::ports::ProjectionWriter> = Arc::new(stores.projections);

    let service: Arc<dyn sem_os_core::service::CoreService> = Arc::new(
        CoreServiceImpl::new(
            Arc::new(stores.snapshots),
            Arc::new(stores.objects),
            Arc::new(stores.changesets),
            Arc::new(stores.audit),
            Arc::clone(&outbox),
            Arc::new(stores.evidence),
            Arc::clone(&projections),
        )
        .with_authoring(Arc::new(stores.authoring))
        .with_scratch_runner(Arc::new(stores.scratch_runner))
        .with_cleanup(Arc::new(stores.cleanup))
        .with_bootstrap_audit(Arc::new(stores.bootstrap_audit) as Arc<dyn BootstrapAuditStore>),
    );

    let jwt_config = JwtConfig::from_secret(TEST_JWT_SECRET);
    build_router(service, jwt_config)
}

fn sample_propose_body() -> serde_json::Value {
    let manifest_yaml = r#"
title: "Test ChangeSet"
rationale: "Integration test fixture"
depends_on: []
supersedes: null
artifacts:
  - type: migration_sql
    path: "001_test.sql"
    ordinal: 0
"#;
    let mut artifacts = HashMap::new();
    artifacts.insert(
        "001_test.sql".to_string(),
        "SELECT 1; -- integration test".to_string(),
    );
    serde_json::json!({
        "manifest_yaml": manifest_yaml,
        "artifacts": artifacts,
    })
}

// ── Helper to read response body ───────────────────────────────

async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or_else(
        |_| serde_json::json!({ "raw": String::from_utf8_lossy(&bytes).to_string() }),
    )
}

// ── Tests ───────────────────────────────────────────────────────

#[tokio::test]
#[ignore] // requires DATABASE_URL
async fn test_health_no_auth() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore] // requires DATABASE_URL
async fn test_propose_requires_auth() {
    let app = build_test_app().await;
    // No Authorization header → 401
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/authoring/propose")
                .header("content-type", "application/json")
                .body(Body::from(sample_propose_body().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore] // requires DATABASE_URL
async fn test_propose_with_valid_jwt() {
    let app = build_test_app().await;
    let token = admin_governed_jwt();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/authoring/propose")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(sample_propose_body().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    // Should succeed (200) or return a known error — NOT 401/403
    let status = resp.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::CONFLICT,
        "Expected 200 or 409, got {status}"
    );
}

#[tokio::test]
#[ignore] // requires DATABASE_URL
async fn test_publish_requires_admin() {
    let app = build_test_app().await;
    let token = viewer_governed_jwt(); // viewer, NOT admin
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/authoring/00000000-0000-0000-0000-000000000001/publish")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({"publisher": "attacker"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = body_json(resp).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap_or("")
            .contains("not an admin"),
        "Expected admin rejection, got: {body}"
    );
}

#[tokio::test]
#[ignore] // requires DATABASE_URL
async fn test_publish_blocked_in_research_mode() {
    let app = build_test_app().await;
    let token = admin_research_jwt(); // admin but Research mode
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/authoring/00000000-0000-0000-0000-000000000001/publish")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({"publisher": "test-admin"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = body_json(resp).await;
    assert!(
        body["error"].as_str().unwrap_or("").contains("AgentMode"),
        "Expected AgentMode rejection, got: {body}"
    );
}

#[tokio::test]
#[ignore] // requires DATABASE_URL
async fn test_list_changesets_works_in_both_modes() {
    // Read-only endpoints should work in any mode
    for (label, token) in [
        ("governed-admin", admin_governed_jwt()),
        ("research-admin", admin_research_jwt()),
        ("governed-viewer", viewer_governed_jwt()),
    ] {
        let app = build_test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/authoring")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "GET /authoring failed for {label}: {}",
            resp.status()
        );
    }
}

#[tokio::test]
#[ignore] // requires DATABASE_URL
async fn test_tools_routes_return_404() {
    let app = build_test_app().await;
    let token = admin_governed_jwt();

    // /tools/list should be 404 (removed)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/tools/list")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "GET /tools/list should return 404, got {}",
        resp.status()
    );

    // /tools/call should be 404 (removed)
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tools/call")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "POST /tools/call should return 404, got {}",
        resp.status()
    );
}

#[tokio::test]
#[ignore] // requires DATABASE_URL
async fn test_get_changeset_by_id_routes_correctly() {
    let app = build_test_app().await;
    let token = admin_governed_jwt();
    // GET /authoring/{id} should route (even if the UUID doesn't exist, we expect 404 from handler, not from router)
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/authoring/00000000-0000-0000-0000-000000000001")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = body_json(resp).await;
    eprintln!("GET /authoring/{{id}} status={status} body={body}");
    // If the route matched, the handler returns JSON body with "error" field (from AppError).
    // If the route did NOT match, we get an empty body (router's default 404).
    let has_error_field = body.get("error").is_some();
    assert!(
        has_error_field,
        "Expected handler JSON error (route matched) but got bare 404 (route not matched). body={body}"
    );
}

/// Minimal routing probe: tests that a simple `{id}` parametric route resolves.
#[tokio::test]
#[ignore] // requires DATABASE_URL
async fn test_changeset_parametric_route_probe() {
    let app = build_test_app().await;
    let token = admin_governed_jwt();

    // Test /changesets/{id}/diff — another parametric route
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/changesets/00000000-0000-0000-0000-000000000001/diff")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = body_json(resp).await;
    eprintln!("GET /changesets/{{id}}/diff status={status} body={body}");

    // Test /authoring/{id}/validate — POST
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/authoring/00000000-0000-0000-0000-000000000001/validate")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status2 = resp.status();
    let body2 = body_json(resp).await;
    eprintln!("POST /authoring/{{id}}/validate status={status2} body={body2}");
}

#[tokio::test]
#[ignore] // requires DATABASE_URL
async fn test_publish_batch_blocked_for_non_admin() {
    let app = build_test_app().await;
    let token = viewer_governed_jwt();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/authoring/publish-batch")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({
                        "change_set_ids": [],
                        "publisher": "attacker"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
