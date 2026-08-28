//! Seam tests.
//!
//! - `into_event` mapping is a pure unit test (no DB).
//! - `append_in_scope` is proven through a REAL `TransactionScope` impl against
//!   live Postgres: the event commits / rolls back with the scope, and a
//!   lexicon precondition checked through the seam rejects under the lock.

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use ob_poc_kyc_seam::{append_in_scope, map_principal, IntentEventDraft};
use ob_poc_kyc_substrate::{
    check_control_preconditions, assembly_lexicon, AuthorityRef, ControlState, FoldRegistry,
    IdemKey, SubjectId, TargetBinding, V1FoldImpl,
};
use ob_poc_types::TransactionScopeId;
use sem_os_core::principal::Principal as RuntimePrincipal;

// ── Pure unit: identity mapping ────────────────────────────────────────────────

fn runtime_principal(actor_id: &str, roles: &[&str]) -> RuntimePrincipal {
    RuntimePrincipal {
        actor_id: actor_id.to_string(),
        roles: roles.iter().map(|s| s.to_string()).collect(),
        claims: HashMap::new(),
        tenancy: None,
    }
}

#[test]
fn into_event_maps_identity_deterministically() {
    let subject = SubjectId(Uuid::new_v4());
    let correlation = Uuid::new_v4();
    let execution = Uuid::new_v4();
    let p = runtime_principal("alice", &["analyst", "admin"]);

    let draft = || IntentEventDraft {
        verb_fqn: "kyc_ubo.assert.subject.register".into(),
        subject_root: subject,
        target: TargetBinding::for_subject(subject),
        payload: serde_json::json!({ "k": "v" }),
        authority: AuthorityRef("analyst.register".into()),
        lexicon_hash: assembly_lexicon().hash,
        as_of: chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    };

    let e1 = draft().into_event(&p, correlation, execution);
    let e2 = draft().into_event(&p, correlation, execution);

    // Non-UUID actor_id → a STABLE derived UUID (replay/audit reproducible).
    assert_eq!(
        e1.actor.actor_id, e2.actor.actor_id,
        "actor_id mapping is deterministic"
    );
    assert_ne!(
        e1.actor.actor_id,
        Uuid::nil(),
        "derived actor_id is non-nil"
    );
    // Primary role only (substrate principal is single-role).
    assert_eq!(e1.actor.role, "analyst");
    // execution_id is the idempotency key (F); correlation threaded through.
    assert_eq!(e1.idempotency_key, IdemKey::new(execution.to_string()));
    assert_eq!(e1.correlation_id, correlation);

    // A real-UUID actor_id parses through unchanged.
    let real = Uuid::new_v4();
    let pe = runtime_principal(&real.to_string(), &["ops"]);
    assert_eq!(map_principal(&pe).actor_id, real);
    // Empty role vec → empty primary role, not a panic.
    let pn = runtime_principal("svc", &[]);
    assert_eq!(map_principal(&pn).role, "");
}

// ── Live DB: append through a real TransactionScope ────────────────────────────

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
    r
}

/// A minimal test `TransactionScope` wrapping a pool transaction. Mirrors the
/// production `PgTransactionScope` shape the Sequencer supplies (tx + pool + id).
struct TestScope {
    tx: Transaction<'static, Postgres>,
    pool: PgPool,
    id: TransactionScopeId,
}

impl TestScope {
    async fn begin(pool: &PgPool) -> Self {
        Self {
            tx: pool.begin().await.unwrap(),
            pool: pool.clone(),
            id: TransactionScopeId::new(),
        }
    }
    async fn commit(self) {
        self.tx.commit().await.unwrap();
    }
    async fn rollback(self) {
        self.tx.rollback().await.unwrap();
    }
}

impl TransactionScope for TestScope {
    fn scope_id(&self) -> TransactionScopeId {
        self.id
    }
    fn transaction(&mut self) -> &mut Transaction<'static, Postgres> {
        &mut self.tx
    }
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}

fn register_event(subject: SubjectId, idem: &str) -> ob_poc_kyc_substrate::IntentEvent {
    IntentEventDraft {
        verb_fqn: "kyc_ubo.assert.subject.register".into(),
        subject_root: subject,
        target: TargetBinding::for_subject(subject),
        payload: serde_json::json!({ "is_natural_person": false }),
        authority: AuthorityRef("analyst.register".into()),
        lexicon_hash: assembly_lexicon().hash,
        as_of: chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    }
    .into_event(
        &runtime_principal("analyst-1", &["analyst"]),
        Uuid::new_v4(),
        Uuid::new_v4(),
    )
    .with_idempotency_key(IdemKey::new(idem))
}

async fn count(pool: &PgPool, subject: SubjectId) -> i64 {
    sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#)
        .bind(subject.0)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn cleanup(pool: &PgPool, subject: SubjectId) {
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#)
        .bind(subject.0)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".kyc_subject_streams WHERE subject_root = $1"#)
        .bind(subject.0)
        .execute(pool)
        .await;
}

#[tokio::test]
async fn append_in_scope_commits_and_rolls_back_with_the_scope() {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("connect to test DB");
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let event = register_event(subject, "scope-tx");

    // Append inside a scope, then ROLL BACK the scope → the event is gone.
    {
        let mut scope = TestScope::begin(&pool).await;
        append_in_scope(&mut scope, &registry, &event, "(test-event)", |_, _, _| Ok(()))
            .await
            .unwrap();
        scope.rollback().await;
    }
    assert_eq!(
        count(&pool, subject).await,
        0,
        "rollback of the scope dropped the appended event"
    );

    // Append inside a scope, then COMMIT the scope → the event persists.
    {
        let mut scope = TestScope::begin(&pool).await;
        let outcome = append_in_scope(&mut scope, &registry, &event, "(test-event)", |_, _, _| Ok(()))
            .await
            .unwrap();
        assert_eq!(outcome.seq, 0);
        scope.commit().await;
    }
    assert_eq!(
        count(&pool, subject).await,
        1,
        "commit of the scope persisted the event"
    );

    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn lexicon_precondition_rejects_through_the_seam() {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("connect to test DB");
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let lexicon = assembly_lexicon();
    // EOP-DD-UBO-PROOF-001 §3/§4 (T5, 2026-08-28): `kyc_ubo.assert.edge.
    // verification` RETIRED (K-G7: 0 real committed events) — no ratchet
    // left to reject a premature step onto. `disconnect`'s `EdgeExists`
    // stud is the replacement precondition-through-the-seam proof: an
    // edge that was never asserted cannot be disconnected.
    let disconnect_entry = lexicon
        .get("kyc_ubo.assert.edge.disconnect")
        .expect("disconnect entry in lexicon");

    // A disconnect event for an edge that was never asserted in the (empty) stream.
    let edge = Uuid::new_v4();
    let disconnect_event = IntentEventDraft {
        verb_fqn: "kyc_ubo.assert.edge.disconnect".into(),
        subject_root: subject,
        target: TargetBinding::for_edge(subject, ob_poc_kyc_substrate::EdgeId(edge)),
        payload: serde_json::json!({}),
        authority: AuthorityRef("analyst.disconnect".into()),
        lexicon_hash: lexicon.hash,
        as_of: chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    }
    .into_event(
        &runtime_principal("analyst-1", &["analyst"]),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );

    let mut scope = TestScope::begin(&pool).await;
    let result = append_in_scope(
        &mut scope,
        &registry,
        &disconnect_event,
        "(test-event)",
        |state: &ControlState,
         _obligation: &ob_poc_kyc_substrate::ObligationState,
         type_registry: &ob_poc_kyc_substrate::TypeRegistryState| {
            check_control_preconditions(disconnect_entry, state, type_registry, &disconnect_event)
        },
    )
    .await;
    assert!(
        result.is_err(),
        "disconnect of an edge that was never asserted must be rejected through the seam"
    );
    scope.rollback().await;

    assert_eq!(
        count(&pool, subject).await,
        0,
        "rejected append inserts nothing"
    );
    cleanup(&pool, subject).await;
}
