//! P0 RED gates — 2026-09-07 audit item 3, "a human can place exactly ONE
//! block ever." `EOP-VS-UBO-GAME-001` §3.4 R9: `place` offers the entity
//! TYPES that may be added; which specific entity is a lookup, supplied by
//! the caller, never pre-enumerated.
//!
//! `a_human_can_build_a_two_block_board` is the audit's own probe (2026-08-28
//! finding, item 3) turned into a permanent gate: through the WORKBOOK
//! surface only (`KycWorkbook::stage`, no Rust-level escape hatch), place
//! the subject, place a brand-new second entity, then connect them. RED
//! today — `enumerate_placement_set`'s `place` branch only ever offers the
//! subject's own entity id and previously-withdrawn members (`placement.rs`
//! doc comment, T1's P4 deferral); a never-before-seen entity id has no
//! candidate to match against, so `stage()`'s target-equality frontier match
//! refuses it with `NotCurrentlyLegal`.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use ob_poc::domain_ops::kyc_workbook::open_workbook;
use ob_poc_kyc_substrate::SubjectId;
use ob_poc_types::TransactionScopeId;
use sem_os_core::principal::Principal as RuntimePrincipal;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn connect_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("connect to test DB")
}

fn runtime_principal() -> RuntimePrincipal {
    RuntimePrincipal {
        actor_id: "kyc-place-frontier-gate".to_string(),
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
        Self { tx: pool.begin().await.unwrap(), pool: pool.clone(), id: TransactionScopeId::new() }
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

/// The audit's item-3 probe: a human, through the workbook surface alone,
/// must be able to place a SECOND, brand-new block and connect it to the
/// first. One block is not a board.
#[tokio::test]
async fn a_human_can_build_a_two_block_board() {
    let pool = connect_pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let subject_uuid = subject.0;
    let second = Uuid::new_v4();

    let mut scope = TestScope::begin(&pool).await;
    let mut workbook = open_workbook(scope.transaction(), subject).await.unwrap();

    workbook
        .stage(
            &format!(
                r#"(kyc_ubo.assert.subject.place :subject-id "{subject_uuid}" :entity-id "{subject_uuid}" :entity-type "private_limited_company")"#
            ),
            &runtime_principal(),
            ob_poc_kyc_substrate::AuthorityRef("gate".into()),
            fixed_ts(),
        )
        .expect("staging the subject's own placement must always be legal (bootstrap)");

    let second_place = workbook.stage(
        &format!(
            r#"(kyc_ubo.assert.subject.place :subject-id "{subject_uuid}" :entity-id "{second}" :entity-type "natural_person")"#
        ),
        &runtime_principal(),
        ob_poc_kyc_substrate::AuthorityRef("gate".into()),
        fixed_ts(),
    );
    assert!(
        second_place.is_ok(),
        "a human must be able to place a SECOND, never-before-seen entity through the \
         workbook surface — one block is not a board (2026-08-28 audit item 3); got: \
         {second_place:?}"
    );

    let connected = workbook.stage(
        &format!(
            r#"(kyc_ubo.assert.edge.connect :subject-id "{subject_uuid}" :from_entity_id "{second}" :to_entity_id "{subject_uuid}" :kind "economic_interest" :percentage 100)"#
        ),
        &runtime_principal(),
        ob_poc_kyc_substrate::AuthorityRef("gate".into()),
        fixed_ts(),
    );
    assert!(
        connected.is_ok(),
        "the second block must be connectable to the first through the workbook surface; \
         got: {connected:?}"
    );
}
