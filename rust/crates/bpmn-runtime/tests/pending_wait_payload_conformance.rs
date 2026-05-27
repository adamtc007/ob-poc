//! Cross-impl conformance test for pending-wait payload storage.
//!
//! InMemoryJourneyStore round-trips a Value through a HashMap with no
//! transformation. PostgresJourneyStore round-trips through JSONB, which
//! reorders object keys and normalises numbers — not byte-identical.
//!
//! This test asserts SEMANTIC equality on both impls so divergence between
//! in-memory (used in tests) and Postgres (used in production) is caught
//! before it reaches production.

use bpmn_runtime::{InMemoryJourneyStore, JourneyStore};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

/// Write a pending wait with a payload, read it back, assert semantic equality.
async fn roundtrip_payload_on(store: Arc<dyn JourneyStore>) {
    // Minimal instance + token to satisfy foreign key constraints in Postgres.
    let instance = store
        .create_instance("test-process", json!({}))
        .await
        .expect("create_instance");

    let token = store
        .create_token(instance.id, "test-node", None, vec![])
        .await
        .expect("create_token");

    // The payload mirrors what DslFormHandler emits: { form_ref, mode, prefill_data }.
    // Include a numeric value to exercise JSONB number normalisation.
    let payload = json!({
        "form_ref":     "kyc.review-summary",
        "mode":         "display",
        "prefill_data": {
            "score":    750,
            "customer": "Allianz",
            "nested":   { "flag": true }
        }
    });

    let correlation = token.id.to_string();

    store
        .create_pending_wait(
            instance.id,
            token.id,
            "human_task",
            "test-node",
            Some(correlation.clone()),
            None,
            Some(payload.clone()),
        )
        .await
        .expect("create_pending_wait");

    let info = store
        .find_pending_wait_by_correlation("human_task", &correlation)
        .await
        .expect("find_pending_wait_by_correlation")
        .expect("should find the wait");

    let stored = info.payload.expect("payload should be Some");

    // Semantic equality — key order and number representation may differ.
    assert_eq!(
        stored.get("form_ref"),
        payload.get("form_ref"),
        "form_ref should round-trip"
    );
    assert_eq!(
        stored.get("mode"),
        payload.get("mode"),
        "mode should round-trip"
    );
    let stored_prefill = stored.get("prefill_data").expect("prefill_data present");
    assert_eq!(
        stored_prefill.get("score").and_then(|v| v.as_i64()),
        Some(750),
        "numeric score should round-trip semantically"
    );
    assert_eq!(
        stored_prefill.get("customer").and_then(|v| v.as_str()),
        Some("Allianz"),
        "string customer should round-trip"
    );
    assert_eq!(
        stored_prefill
            .get("nested")
            .and_then(|v| v.get("flag"))
            .and_then(|v| v.as_bool()),
        Some(true),
        "nested bool should round-trip"
    );
}

#[tokio::test]
async fn in_memory_store_payload_roundtrip() {
    let store = Arc::new(InMemoryJourneyStore::new()) as Arc<dyn JourneyStore>;
    roundtrip_payload_on(store).await;
}

// Postgres conformance test — requires DATABASE_URL and the dsl_journey_runtime
// migration applied. Skipped in CI unless the database feature is enabled.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_store_payload_roundtrip() {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for postgres conformance test");
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("connect to postgres");
    let store = Arc::new(bpmn_runtime::PostgresJourneyStore::new(pool)) as Arc<dyn JourneyStore>;
    roundtrip_payload_on(store).await;
}
