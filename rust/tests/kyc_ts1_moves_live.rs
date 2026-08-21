//! D1 Part A — RED-first live-DB gates for the four TS.1 §3 moves newly
//! wired to the real dsl.kyc write path (EOP-DD-KYCUBO-TS.1):
//! `kyc.subject.assert-type` (move 2), `kyc.subject.correct-type` (move 7),
//! `kyc.subject.withdraw-member` (move 6), `kyc.subject.record-enquiry`
//! (move 8). Each move gets an admission pair (a legal position admits, an
//! illegal position refuses) plus a persistence test proving a real append
//! lands and re-folds correctly.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectAssertType, KycSubjectCorrectType, KycSubjectRecordEnquiry, KycSubjectRegister,
    KycSubjectWithdrawMember, UboEdgeAssertControl,
};
use ob_poc_kyc_store::PgKycEventStore;
use ob_poc_kyc_substrate::{fold_type_registry, EntityId, EntityType, SubjectId, TypeProofStatus};
use ob_poc_types::TransactionScopeId;
use sem_os_postgres::ops::SemOsVerbOp;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("connect to test DB")
}

struct Scope {
    tx: Transaction<'static, Postgres>,
    pool: PgPool,
    id: TransactionScopeId,
}
impl Scope {
    async fn begin(p: &PgPool) -> Self {
        Self {
            tx: p.begin().await.unwrap(),
            pool: p.clone(),
            id: TransactionScopeId::new(),
        }
    }
    async fn commit(self) {
        self.tx.commit().await.unwrap();
    }
}
impl TransactionScope for Scope {
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

async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> serde_json::Value {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op
        .execute(&args, &mut ctx, &mut scope)
        .await
        .unwrap_or_else(|error| panic!("{}: {error}", op.fqn()));
    scope.commit().await;
    match out {
        dsl_runtime::VerbExecutionOutcome::Record(v) => v,
        other => serde_json::to_value(format!("{other:?}")).unwrap(),
    }
}

async fn run_fallible(
    op: &dyn SemOsVerbOp,
    args: serde_json::Value,
    pool: &PgPool,
) -> anyhow::Result<serde_json::Value> {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op.execute(&args, &mut ctx, &mut scope).await?;
    scope.commit().await;
    Ok(match out {
        dsl_runtime::VerbExecutionOutcome::Record(v) => v,
        other => serde_json::to_value(format!("{other:?}")).unwrap(),
    })
}

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in [
            "kyc_intent_events",
            "kyc_subject_streams",
            "kyc_control_edge_projection",
            "kyc_obligation_projection",
            "kyc_subject_rollup_projection",
        ] {
            let _ = sqlx::query(&format!(
                r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#
            ))
            .bind(s.0)
            .execute(pool)
            .await;
        }
    }
}

// ── assert-type: admission pair + persistence ─────────────────────────────

#[tokio::test]
async fn assert_type_refuses_unregistered_admits_registered_persists() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let entity = Uuid::new_v4();

    // Illegal: entity never registered.
    let refused = run_fallible(
        &KycSubjectAssertType,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": entity, "entity-type": "private_limited_company",
        }),
        &pool,
    )
    .await;
    assert!(
        refused.is_err(),
        "assert-type must refuse an unregistered entity (EntityRegistered, TS.1 §3 row 2)"
    );

    // Legal: register, then assert-type.
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "entity-id": entity }),
        &pool,
    )
    .await;
    run(
        &KycSubjectAssertType,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": entity, "entity-type": "private_limited_company",
        }),
        &pool,
    )
    .await;

    // Persistence: a real append landed and re-folds to Alleged.
    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = PgKycEventStore::load_events(&mut conn, subject).await.unwrap();
    let refs: Vec<_> = events.iter().collect();
    let type_registry = fold_type_registry(&refs);
    let record = type_registry
        .types
        .get(&EntityId(entity))
        .expect("assert-type must persist a type record");
    assert_eq!(record.entity_type, EntityType::PrivateLimitedCompany);
    assert_eq!(record.proof, TypeProofStatus::Alleged);

    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn assert_type_rejects_unknown_wire_value_fail_closed() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let entity = Uuid::new_v4();
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "entity-id": entity }),
        &pool,
    )
    .await;

    let refused = run_fallible(
        &KycSubjectAssertType,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": entity, "entity-type": "not_a_real_type",
        }),
        &pool,
    )
    .await;
    assert!(refused.is_err(), "assert-type must reject an unrecognized entity-type fail-closed");

    cleanup(&pool, &[subject]).await;
}

// ── correct-type: admission pair + cascade persistence ────────────────────

#[tokio::test]
async fn correct_type_refuses_untyped_admits_typed_and_cascades() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let company = Uuid::new_v4();
    let holder = Uuid::new_v4();

    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "entity-id": company }),
        &pool,
    )
    .await;
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "entity-id": holder }),
        &pool,
    )
    .await;

    // Illegal: no prior type assertion for `company`.
    let refused = run_fallible(
        &KycSubjectCorrectType,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": company, "entity-type": "general_partnership",
        }),
        &pool,
    )
    .await;
    assert!(
        refused.is_err(),
        "correct-type must refuse an entity with no prior type assertion"
    );

    // Type both ends, assert a voting-shares control edge holder -> company
    // (permitted into a private limited company, TS.1 §2).
    run(
        &KycSubjectAssertType,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": company, "entity-type": "private_limited_company",
        }),
        &pool,
    )
    .await;
    run(
        &KycSubjectAssertType,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": holder, "entity-type": "natural_person",
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": holder, "to_entity_id": company,
            "kind": "voting_rights",
        }),
        &pool,
    )
    .await;

    // Legal: correct company's type to general_partnership — voting_rights
    // is not a permitted pipe into a general partnership (TS.1 §2), so the
    // edge must be invalidated by the cascade, never deleted.
    let result = run(
        &KycSubjectCorrectType,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": company, "entity-type": "general_partnership",
        }),
        &pool,
    )
    .await;
    let invalidated_count = result["invalidated_edge_count"].as_u64().unwrap();
    assert_eq!(invalidated_count, 1, "the voting-shares edge must be invalidated by the cascade");

    // Persistence: the correction and the flagged edge both re-fold.
    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = PgKycEventStore::load_events(&mut conn, subject).await.unwrap();
    let refs: Vec<_> = events.iter().collect();
    let type_registry = fold_type_registry(&refs);
    assert_eq!(type_registry.corrections.len(), 1);
    assert!(type_registry.determination_stale);
    assert_eq!(type_registry.flagged_edges.len(), 1);
    assert_eq!(
        type_registry.type_of(EntityId(company)),
        Some(EntityType::GeneralPartnership)
    );

    cleanup(&pool, &[subject]).await;
}

// ── withdraw-member: admission pair (no double-withdraw) + persistence ────

#[tokio::test]
async fn withdraw_member_refuses_unregistered_admits_once_refuses_twice() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let entity = Uuid::new_v4();

    // Illegal: never registered.
    let refused = run_fallible(
        &KycSubjectWithdrawMember,
        serde_json::json!({ "subject-id": subject.0, "entity-id": entity }),
        &pool,
    )
    .await;
    assert!(refused.is_err(), "withdraw-member must refuse an unregistered entity");

    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "entity-id": entity }),
        &pool,
    )
    .await;

    // Legal: registered and not yet withdrawn.
    run(
        &KycSubjectWithdrawMember,
        serde_json::json!({ "subject-id": subject.0, "entity-id": entity }),
        &pool,
    )
    .await;

    // Illegal again: already withdrawn (membership must be active).
    let re_withdraw = run_fallible(
        &KycSubjectWithdrawMember,
        serde_json::json!({ "subject-id": subject.0, "entity-id": entity }),
        &pool,
    )
    .await;
    assert!(re_withdraw.is_err(), "withdraw-member must refuse a second withdrawal");

    // Persistence.
    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = PgKycEventStore::load_events(&mut conn, subject).await.unwrap();
    let refs: Vec<_> = events.iter().collect();
    let type_registry = fold_type_registry(&refs);
    assert!(type_registry.is_withdrawn(EntityId(entity)));

    cleanup(&pool, &[subject]).await;
}

// ── record-enquiry: always admitted + persistence ──────────────────────────

#[tokio::test]
async fn record_enquiry_admitted_without_registration_and_persists() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    // Legal even with nothing else on the stream — "group exists" is true
    // by construction (TS.1 §3 row 8).
    run(
        &KycSubjectRecordEnquiry,
        serde_json::json!({
            "subject-id": subject.0,
            "sources-consulted": ["GLEIF", "companies-house"],
            "searches-run": ["group structure search"],
        }),
        &pool,
    )
    .await;

    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = PgKycEventStore::load_events(&mut conn, subject).await.unwrap();
    let refs: Vec<_> = events.iter().collect();
    let type_registry = fold_type_registry(&refs);
    assert_eq!(type_registry.enquiries.len(), 1);
    assert_eq!(
        type_registry.enquiries[0].sources_consulted,
        vec!["GLEIF".to_string(), "companies-house".to_string()]
    );
    assert_eq!(
        type_registry.enquiries[0].searches_run,
        vec!["group structure search".to_string()]
    );

    cleanup(&pool, &[subject]).await;
}
