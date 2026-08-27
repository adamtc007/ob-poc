//! T2 gate tests — `EOP-VS-UBO-GAME-001` §3.1/§3.2/§8 Q1.
//!
//! P0c RED gates, written before `place`/`remove` exist. At RED time all
//! three fail at the same point (`canonical_event_shape` bails on the
//! unrecognized `kyc_ubo.assert.subject.place`/`.remove` FQNs) — the
//! earliest failure point available, and the correct one: neither verb has
//! any surface yet.
//!
//! **Why these tests place the SUBJECT's own entity, not a distinct one**
//! (P2 finding, reported before implementing — state-of-play §5n): `place`'s
//! board candidates are entity-scoped (needed to make "already placed" a
//! real, non-vacuous board property — T1's P4 deferral, closed here for the
//! re-placeable population). The only entities the board can enumerate as
//! `place` candidates are (a) the subject's own not-yet-placed entity — a
//! real, known singleton, no discovery data needed — and (b) every
//! currently-withdrawn (previously-placed) entity. A brand-new, non-subject
//! entity's first-ever placement has no board candidate to match against
//! (no group/discovery data source exists), so it is no longer reachable
//! through `KycWorkbook::stage()` — only through the op-layer, which has no
//! frontier/board gate, only `check_preconditions`. These gates exercise
//! the two genuinely board-enumerable cases; `place_records_entity_and_type_in_one_move`
//! additionally proves the op-layer path directly (`canonical_event_shape`)
//! against a distinct entity, since that path needs no board candidate.
//!
//! - `place_records_entity_and_type_in_one_move` — through BOTH surfaces,
//!   one committed move puts an entity on the board AND gives it a type.
//! - `remove_withdraws_the_placement_not_the_entity` — §2: "the game
//!   writes metadata about entities... `remove` withdraws a placement,
//!   never an entity." After `remove`, the entity is still a group member
//!   (never deleted) and can be placed again.
//! - `type_correction_is_remove_then_place` — §8 Q1 (RULED 2026-08-27):
//!   correcting a type is `remove` then `place`, two moves, not a
//!   superseding placement. The stream shows both.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use ob_poc::domain_ops::kyc_workbook::open_workbook;
use ob_poc_kyc_seam::canonical_event_shape;
use ob_poc_kyc_substrate::{assembly_lexicon, EntityId, FoldRegistry, SubjectId, V1FoldImpl};
use ob_poc_types::TransactionScopeId;
use sem_os_core::principal::Principal as RuntimePrincipal;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn connect() -> PgPool {
    PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url())
        .await
        .expect("connect to test DB")
}

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
    r
}

fn runtime_principal(actor_id: &str) -> RuntimePrincipal {
    RuntimePrincipal {
        actor_id: actor_id.to_string(),
        roles: vec!["analyst".to_string()],
        claims: std::collections::HashMap::new(),
        tenancy: None,
    }
}

fn fixed_ts() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

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

// ── place_records_entity_and_type_in_one_move ───────────────────────────────

#[tokio::test]
async fn place_records_entity_and_type_in_one_move() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let other_entity = Uuid::new_v4();
    let registry = v1_registry();
    let principal = runtime_principal("analyst-1");
    let as_of = fixed_ts();

    // Op-layer path: a distinct entity — no board gate on this path, only
    // canonical_event_shape needs to know the verb.
    let args = serde_json::json!({
        "entity-id": other_entity.to_string(),
        "entity-type": "private_limited_company",
    });
    let op_result = canonical_event_shape("kyc_ubo.assert.subject.place", subject, &args);

    // Workbook-layer path: the subject's own entity — the one board
    // candidate that needs no discovery data (see module doc).
    let mut conn = pool.acquire().await.unwrap();
    let mut workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);
    let stage_result = workbook.stage(
        &format!(
            r#"(kyc_ubo.assert.subject.place :subject-id "{subject}" :entity-id "{subject}" :entity-type "general_partnership")"#,
            subject = subject.0
        ),
        &principal,
        ob_poc_kyc_substrate::AuthorityRef("analyst.place".into()),
        as_of,
    );

    if op_result.is_err() && stage_result.is_err() {
        panic!(
            "RED (expected pre-T2): neither surface knows kyc_ubo.assert.subject.place yet — \
             op-layer: {:?}, workbook: {:?}",
            op_result.err(),
            stage_result.err()
        );
    }

    // GREEN path (post-T2): both surfaces built it.
    let (op_target, op_payload, _) = op_result.expect("op-layer canonical_event_shape must build place");
    assert_eq!(op_target.entity_id, Some(EntityId(other_entity)));
    assert_eq!(op_payload["entity_type"], "private_limited_company");

    stage_result.expect("workbook stage must build place");
    let mut scope = TestScope::begin(&pool).await;
    workbook
        .commit(&mut scope, &registry)
        .await
        .expect("place has no preconditions that would block a first placement");
    scope.commit().await;

    let mut conn2 = pool.acquire().await.unwrap();
    let reopened = open_workbook(&mut conn2, subject).await.unwrap();
    assert_eq!(
        reopened.committed.len(),
        1,
        "one DSL move must produce exactly one stream event"
    );
    let (control, _obligation, type_registry) = reopened.validate().unwrap();
    let subject_entity = EntityId(subject.0);
    assert!(
        control.registered_entity_ids.contains(&subject_entity),
        "place must record group membership"
    );
    assert_eq!(
        type_registry.type_of(subject_entity),
        Some(ob_poc_kyc_substrate::EntityType::GeneralPartnership),
        "place must record the type, in the same move"
    );

    cleanup(&pool, subject).await;
}

// ── remove_withdraws_the_placement_not_the_entity ───────────────────────────

#[tokio::test]
async fn remove_withdraws_the_placement_not_the_entity() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let subject_entity = EntityId(subject.0);
    let registry = v1_registry();
    let principal = runtime_principal("analyst-1");
    let as_of = fixed_ts();

    let mut conn = pool.acquire().await.unwrap();
    let mut workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);

    let place_result = workbook.stage(
        &format!(
            r#"(kyc_ubo.assert.subject.place :subject-id "{subject}" :entity-id "{subject}" :entity-type "private_limited_company")"#,
            subject = subject.0
        ),
        &principal,
        ob_poc_kyc_substrate::AuthorityRef("analyst.place".into()),
        as_of,
    );
    if place_result.is_err() {
        panic!(
            "RED (expected pre-T2): kyc_ubo.assert.subject.place not yet buildable — {:?}",
            place_result.err()
        );
    }
    let mut scope1 = TestScope::begin(&pool).await;
    workbook.commit(&mut scope1, &registry).await.expect("place commits");
    scope1.commit().await;

    let mut conn2 = pool.acquire().await.unwrap();
    let mut workbook2 = open_workbook(&mut conn2, subject).await.unwrap();
    drop(conn2);
    workbook2
        .stage(
            &format!(
                r#"(kyc_ubo.assert.subject.remove :subject-id "{subject}" :entity-id "{subject}")"#,
                subject = subject.0
            ),
            &principal,
            ob_poc_kyc_substrate::AuthorityRef("analyst.remove".into()),
            as_of,
        )
        .expect("remove must be a legal move against a placed entity");
    let mut scope2 = TestScope::begin(&pool).await;
    workbook2.commit(&mut scope2, &registry).await.expect("remove commits");
    scope2.commit().await;

    let mut conn3 = pool.acquire().await.unwrap();
    let reopened = open_workbook(&mut conn3, subject).await.unwrap();
    let (control, _obligation, type_registry) = reopened.validate().unwrap();
    assert!(
        control.registered_entity_ids.contains(&subject_entity),
        "§2: remove withdraws a placement, never an entity — the entity must remain a \
         group member"
    );
    assert!(
        type_registry.is_withdrawn(subject_entity),
        "remove must flag the placement as withdrawn"
    );

    // Re-place: still a group member, and now withdrawn, so the board must
    // offer it again — a real, non-vacuous candidate (T1's P4 closed here).
    let mut workbook3 = reopened;
    let replace_result = workbook3.stage(
        &format!(
            r#"(kyc_ubo.assert.subject.place :subject-id "{subject}" :entity-id "{subject}" :entity-type "public_listed_company")"#,
            subject = subject.0
        ),
        &principal,
        ob_poc_kyc_substrate::AuthorityRef("analyst.replace".into()),
        as_of,
    );
    replace_result.expect(
        "§2: the entity is untouched and remains available to be placed again after remove",
    );

    cleanup(&pool, subject).await;
}

// ── type_correction_is_remove_then_place ────────────────────────────────────

#[tokio::test]
async fn type_correction_is_remove_then_place() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let subject_entity = EntityId(subject.0);
    let registry = v1_registry();
    let principal = runtime_principal("analyst-1");
    let as_of = fixed_ts();

    let mut conn = pool.acquire().await.unwrap();
    let mut workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);
    let place_result = workbook.stage(
        &format!(
            r#"(kyc_ubo.assert.subject.place :subject-id "{subject}" :entity-id "{subject}" :entity-type "general_partnership")"#,
            subject = subject.0
        ),
        &principal,
        ob_poc_kyc_substrate::AuthorityRef("analyst.place".into()),
        as_of,
    );
    if place_result.is_err() {
        panic!(
            "RED (expected pre-T2): kyc_ubo.assert.subject.place not yet buildable — {:?}",
            place_result.err()
        );
    }
    let mut scope1 = TestScope::begin(&pool).await;
    workbook.commit(&mut scope1, &registry).await.expect("initial place commits");
    scope1.commit().await;

    // §8 Q1: correction is remove then place — two moves, not an update.
    let mut conn2 = pool.acquire().await.unwrap();
    let mut workbook2 = open_workbook(&mut conn2, subject).await.unwrap();
    drop(conn2);
    workbook2
        .stage(
            &format!(
                r#"(kyc_ubo.assert.subject.remove :subject-id "{subject}" :entity-id "{subject}")"#,
                subject = subject.0
            ),
            &principal,
            ob_poc_kyc_substrate::AuthorityRef("analyst.correct".into()),
            as_of,
        )
        .expect("remove, half one of the correction");
    workbook2
        .stage(
            &format!(
                r#"(kyc_ubo.assert.subject.place :subject-id "{subject}" :entity-id "{subject}" :entity-type "limited_partnership")"#,
                subject = subject.0
            ),
            &principal,
            ob_poc_kyc_substrate::AuthorityRef("analyst.correct".into()),
            as_of,
        )
        .expect("place, half two of the correction — legal because remove already fired");
    let mut scope2 = TestScope::begin(&pool).await;
    workbook2
        .commit(&mut scope2, &registry)
        .await
        .expect("the two-move correction sequence commits together");
    scope2.commit().await;

    let mut conn3 = pool.acquire().await.unwrap();
    let reopened = open_workbook(&mut conn3, subject).await.unwrap();
    assert_eq!(
        reopened.committed.len(),
        3,
        "the stream must show all three events: initial place, remove, corrective place"
    );
    assert_eq!(reopened.committed[0].verb_fqn.as_str(), "kyc_ubo.assert.subject.place");
    assert_eq!(reopened.committed[1].verb_fqn.as_str(), "kyc_ubo.assert.subject.remove");
    assert_eq!(reopened.committed[2].verb_fqn.as_str(), "kyc_ubo.assert.subject.place");
    let (_control, _obligation, type_registry) = reopened.validate().unwrap();
    assert_eq!(
        type_registry.type_of(subject_entity),
        Some(ob_poc_kyc_substrate::EntityType::LimitedPartnership),
        "the corrected type must be the one that lands"
    );

    cleanup(&pool, subject).await;
}
