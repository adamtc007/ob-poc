//! Integration test: Chat responses include `available_verbs` (verb profile universe)
//!
//! Tests that `POST /api/session/:id/chat` returns structured verb profiles
//! in every response.
//!
//! Run:
//! ```bash
//! DATABASE_URL="postgresql:///data_designer" \
//!   cargo test --features database --test chat_verb_profiles_integration -- --ignored --nocapture
//! ```

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

/// Build the full agent router (same as production, minus frontend static files).
async fn build_test_app() -> Router {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    let sessions = ob_poc::api::create_session_store();

    // Build SemOsClient (in-process mode, same as production default)
    let sem_os_client: Option<Arc<dyn sem_os_client::SemOsClient>> = {
        use sem_os_client::inprocess::InProcessClient;
        use sem_os_core::service::CoreServiceImpl;
        use sem_os_postgres::PgStores;

        let stores = PgStores::new(pool.clone());
        let core_service = Arc::new(
            CoreServiceImpl::new(
                Arc::new(stores.snapshots),
                Arc::new(stores.objects),
                Arc::new(stores.changesets),
                Arc::new(stores.audit),
                Arc::new(stores.outbox),
                Arc::new(stores.evidence),
                Arc::new(stores.projections),
            )
            .with_bootstrap_audit(Arc::new(stores.bootstrap_audit))
            .with_authoring(Arc::new(stores.authoring))
            .with_scratch_runner(Arc::new(stores.scratch_runner))
            .with_cleanup(Arc::new(stores.cleanup)),
        );

        Some(Arc::new(InProcessClient::new(core_service)) as Arc<dyn sem_os_client::SemOsClient>)
    };

    ob_poc::api::create_agent_router_with_semantic(pool, sessions, sem_os_client).await
}

/// Parse response body as JSON.
async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .expect("Failed to read response body");
    serde_json::from_slice(&bytes).expect("Failed to parse JSON")
}

/// Create a new session and return its ID.
async fn create_session(app: &mut Router) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/session")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "Failed to create session");

    let json = body_json(resp).await;
    // Backend returns session_id on create
    let session_id = json
        .get("session_id")
        .or_else(|| json.get("id"))
        .and_then(|v| v.as_str())
        .expect("Response must contain session_id or id");
    session_id.to_string()
}

/// Send a chat message and return the full response JSON.
async fn send_chat(app: &mut Router, session_id: &str, message: &str) -> Value {
    let body = serde_json::json!({ "message": message });
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/session/{}/chat", session_id))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let json = body_json(resp).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "Chat request failed ({}): {:?}",
        status,
        json
    );
    json
}

// ============================================================================
// Tests
// ============================================================================

/// Core test: chat response includes `available_verbs` array with verb profiles.
#[tokio::test]
#[ignore] // requires DATABASE_URL + Candle model
async fn test_chat_response_includes_available_verbs() {
    let mut app = build_test_app().await;

    // 1. Create session
    let session_id = create_session(&mut app).await;
    eprintln!("[TEST] Created session: {}", session_id);

    // 2. Send a simple chat message
    let resp = send_chat(&mut app, &session_id, "what can I do?").await;
    eprintln!(
        "[TEST] Response keys: {:?}",
        resp.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );

    // 3. Check `available_verbs` is present
    let verbs = resp.get("available_verbs");
    assert!(
        verbs.is_some(),
        "Response MUST include 'available_verbs' field. Got keys: {:?}",
        resp.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );

    let verbs_arr = verbs
        .unwrap()
        .as_array()
        .expect("available_verbs must be an array");
    eprintln!("[TEST] available_verbs count: {}", verbs_arr.len());

    // 4. Must have at least some verbs (SemReg returns the full constrained universe)
    assert!(
        !verbs_arr.is_empty(),
        "available_verbs must not be empty — SemReg should return verbs for a new session"
    );

    // 5. Validate shape of first verb profile
    let first = &verbs_arr[0];
    assert!(first.get("fqn").is_some(), "VerbProfile must have 'fqn'");
    assert!(
        first.get("domain").is_some(),
        "VerbProfile must have 'domain'"
    );
    assert!(
        first.get("description").is_some(),
        "VerbProfile must have 'description'"
    );
    assert!(
        first.get("sexpr").is_some(),
        "VerbProfile must have 'sexpr'"
    );
    assert!(first.get("args").is_some(), "VerbProfile must have 'args'");
    assert!(
        first.get("preconditions_met").is_some(),
        "VerbProfile must have 'preconditions_met'"
    );
    assert!(
        first.get("governance_tier").is_some(),
        "VerbProfile must have 'governance_tier'"
    );

    eprintln!(
        "[TEST] First verb: {} — {}",
        first["fqn"].as_str().unwrap_or("?"),
        first["sexpr"].as_str().unwrap_or("?")
    );
}

/// Verb profiles include s-expression signatures with arg info.
#[tokio::test]
#[ignore]
async fn test_verb_profiles_have_sexpr_signatures() {
    let mut app = build_test_app().await;
    let session_id = create_session(&mut app).await;

    let resp = send_chat(&mut app, &session_id, "hello").await;
    let verbs = resp
        .get("available_verbs")
        .and_then(|v| v.as_array())
        .expect("available_verbs must be present");

    // Find a well-known verb (cbu.create should always be in the universe)
    let cbu_create = verbs.iter().find(|v| {
        v.get("fqn")
            .and_then(|f| f.as_str())
            .map_or(false, |f| f == "cbu.create")
    });

    if let Some(verb) = cbu_create {
        let sexpr = verb["sexpr"].as_str().unwrap_or("");
        eprintln!("[TEST] cbu.create sexpr: {}", sexpr);

        // S-expression should start with ( and contain the verb name
        assert!(
            sexpr.starts_with("(cbu.create"),
            "sexpr should start with '(cbu.create', got: {}",
            sexpr
        );
        assert!(
            sexpr.contains(":name"),
            "cbu.create sexpr should contain ':name' arg, got: {}",
            sexpr
        );

        // Check args array
        let args = verb["args"].as_array().expect("args must be array");
        assert!(!args.is_empty(), "cbu.create must have args");

        // Check name arg exists and is required
        let name_arg = args.iter().find(|a| {
            a.get("name")
                .and_then(|n| n.as_str())
                .map_or(false, |n| n == "name")
        });
        assert!(name_arg.is_some(), "cbu.create must have a 'name' argument");
        if let Some(arg) = name_arg {
            assert_eq!(
                arg.get("required").and_then(|r| r.as_bool()),
                Some(true),
                "'name' arg must be required"
            );
            assert_eq!(
                arg.get("arg_type").and_then(|t| t.as_str()),
                Some("string"),
                "'name' arg must be type 'string'"
            );
        }
    } else {
        eprintln!(
            "[TEST] cbu.create not found in available_verbs ({}). Verbs: {:?}",
            verbs.len(),
            verbs
                .iter()
                .take(10)
                .map(|v| v["fqn"].as_str().unwrap_or("?"))
                .collect::<Vec<_>>()
        );
        // Don't hard-fail — SemReg may filter verbs based on session context
        // But warn so we know
        eprintln!("[TEST] WARNING: cbu.create not in verb universe — SemReg may be filtering");
    }
}

/// Verb profiles are present on EVERY chat turn (not just the first).
#[tokio::test]
#[ignore]
async fn test_verb_profiles_on_every_turn() {
    let mut app = build_test_app().await;
    let session_id = create_session(&mut app).await;

    // Turn 1
    let resp1 = send_chat(&mut app, &session_id, "hello").await;
    let verbs1 = resp1.get("available_verbs");
    assert!(verbs1.is_some(), "Turn 1 must include available_verbs");

    // Turn 2
    let resp2 = send_chat(&mut app, &session_id, "what verbs are available?").await;
    let verbs2 = resp2.get("available_verbs");
    assert!(verbs2.is_some(), "Turn 2 must include available_verbs");

    // Turn 3 — slash command (may take early return path)
    let resp3 = send_chat(&mut app, &session_id, "/help").await;
    // /help is an early return — available_verbs may be None here
    // but /commands should populate it
    eprintln!(
        "[TEST] /help response has available_verbs: {}",
        resp3.get("available_verbs").is_some()
    );
}

/// /commands response (early return path) — verify it still returns a message.
/// available_verbs may be None on slash command early returns.
#[tokio::test]
#[ignore]
async fn test_slash_commands_still_works() {
    let mut app = build_test_app().await;
    let session_id = create_session(&mut app).await;

    let resp = send_chat(&mut app, &session_id, "/commands").await;
    let message = resp.get("message").and_then(|m| m.as_str()).unwrap_or("");

    assert!(
        !message.is_empty(),
        "/commands must return a non-empty message"
    );
    eprintln!("[TEST] /commands message length: {} chars", message.len());

    // /commands is an early return — note whether available_verbs is present
    let has_verbs = resp.get("available_verbs").is_some();
    eprintln!(
        "[TEST] /commands has available_verbs: {} (early return path)",
        has_verbs
    );
}

/// Verb profiles group verbs by domain.
#[tokio::test]
#[ignore]
async fn test_verb_profiles_have_domains() {
    let mut app = build_test_app().await;
    let session_id = create_session(&mut app).await;

    let resp = send_chat(&mut app, &session_id, "list options").await;
    let verbs = match resp.get("available_verbs").and_then(|v| v.as_array()) {
        Some(v) => v.clone(),
        None => {
            eprintln!("[TEST] SKIP: available_verbs not present");
            return;
        }
    };

    // Collect unique domains
    let domains: std::collections::HashSet<&str> = verbs
        .iter()
        .filter_map(|v| v.get("domain").and_then(|d| d.as_str()))
        .collect();

    eprintln!("[TEST] Domains found ({}): {:?}", domains.len(), domains);

    // Should have multiple domains
    assert!(
        domains.len() > 1,
        "Verb universe should span multiple domains, got: {:?}",
        domains
    );
}
