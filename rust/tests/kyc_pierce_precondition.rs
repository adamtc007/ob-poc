//! P0/P1/P4 gates — 2026-09-07 audit item 2, "the K-8 pierce guard lives in
//! the op only." `EOP-VS-UBO-GAME-001` §3.4 R7: a rule expressed only in one
//! surface's code is not a rule; preconditions are declared on the move and
//! evaluated at the chokepoint. C2: the board offers only moves the append
//! will admit.
//!
//! `UboEdgeConnect::execute` (`src/domain_ops/kyc_stream_ops.rs`) hand-checks
//! that a `pierced-from` edge exists, is `EdgeKind::Nominee`, and is active —
//! but `KycWorkbook::commit()` never calls that op struct; it drives
//! `append_in_scope` directly with `check_preconditions` as the sole gate
//! (`src/domain_ops/kyc_workbook.rs::commit`). No `Precondition` primitive
//! expressed "this edge is of kind X" (the op's own comment concedes this),
//! so the workbook path had no way to run the op's check even in principle.
//!
//! `pierce_of_non_nominee_edge_through_the_workbook` is the audit's own
//! probe, turned into a permanent gate. At P0 (this file's first commit) it
//! asserted the pierce SUCCEEDS — pasted as proof of the live defect. P1
//! promotes `Precondition::PiercedFromIsActiveNominee` onto `connect`'s
//! lexicon entry and flips this assertion to expect refusal; the flip
//! RED-then-GREENs the same way the op-layer check already did.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use ob_poc::domain_ops::kyc_workbook::open_workbook;
use ob_poc_kyc_substrate::{assembly_lexicon, FoldRegistry, SubjectId, V1FoldImpl};
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

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(assembly_lexicon().hash, std::sync::Arc::new(V1FoldImpl));
    r
}

fn runtime_principal() -> RuntimePrincipal {
    RuntimePrincipal {
        actor_id: "kyc-pierce-precondition-gate".to_string(),
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

/// The audit's item-2 probe: through the workbook alone, cite a genuinely
/// non-nominee edge as `pierced-from`. The referenced edge is `voting_rights`
/// — never asserted as `EdgeKind::Nominee` — so a correct system must refuse.
#[tokio::test]
async fn pierce_of_non_nominee_edge_through_the_workbook() {
    let pool = connect_pool().await;
    let registry = v1_registry();
    let subject = SubjectId(Uuid::new_v4());
    let subject_uuid = subject.0;
    let holder = Uuid::new_v4();
    let far_end = Uuid::new_v4();

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
        .expect("placing the subject must always be legal (bootstrap)");
    workbook
        .stage(
            &format!(
                r#"(kyc_ubo.assert.subject.place :subject-id "{subject_uuid}" :entity-id "{holder}" :entity-type "natural_person")"#
            ),
            &runtime_principal(),
            ob_poc_kyc_substrate::AuthorityRef("gate".into()),
            fixed_ts(),
        )
        .expect("placing the voting-rights holder must be legal");
    workbook
        .stage(
            &format!(
                r#"(kyc_ubo.assert.subject.place :subject-id "{subject_uuid}" :entity-id "{far_end}" :entity-type "private_limited_company")"#
            ),
            &runtime_principal(),
            ob_poc_kyc_substrate::AuthorityRef("gate".into()),
            fixed_ts(),
        )
        .expect("placing the pierce's far end must be legal");

    // A genuine, non-nominee edge: `holder` votes `subject` directly. Never
    // asserted as EdgeKind::Nominee.
    let real_edge = workbook
        .stage(
            &format!(
                r#"(kyc_ubo.assert.edge.connect :subject-id "{subject_uuid}" :from_entity_id "{holder}" :to_entity_id "{subject_uuid}" :kind "voting_rights")"#
            ),
            &runtime_principal(),
            ob_poc_kyc_substrate::AuthorityRef("gate".into()),
            fixed_ts(),
        )
        .expect("staging a real voting_rights edge must be legal");
    let real_edge_id = real_edge
        .event
        .payload
        .get("edge_id")
        .and_then(|v| v.as_str())
        .expect("connect mints edge_id into the payload")
        .to_string();

    // The pierce: cites the VOTING-RIGHTS edge above as `pierced-from` — K-8
    // (`EdgeKind::Nominee` only) is violated on its face.
    workbook
        .stage(
            &format!(
                r#"(kyc_ubo.assert.edge.connect :subject-id "{subject_uuid}" :from_entity_id "{holder}" :to_entity_id "{far_end}" :kind "voting_rights" :pierced-from "{real_edge_id}")"#
            ),
            &runtime_principal(),
            ob_poc_kyc_substrate::AuthorityRef("gate".into()),
            fixed_ts(),
        )
        .expect("staging the pierce must recognise (frontier admits any connect)");

    let commit_result = workbook.commit(&mut scope, &registry).await;
    assert!(
        commit_result.is_err(),
        "K-8 (EOP-VS-UBO-GAME-001 §3.4 R7): the workbook must REFUSE a pierce citing a \
         non-nominee edge, the same rule the op layer already enforces — got: {commit_result:?}"
    );
}
