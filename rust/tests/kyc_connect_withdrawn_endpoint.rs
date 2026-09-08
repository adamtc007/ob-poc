//! P0/P1/P4 gates — EOP-VS-UBO-GAME-001 §3.4, geometry-closure finding #2
//! (fuzz-harness tranche, 2026-09-08). RULED by Adam 2026-09-07: FORBID the
//! connection — both endpoints of `connect` must be actively placed on the
//! board.
//!
//! The fuzzer found: connecting to a WITHDRAWN entity was legal (geometry
//! checks against its type AT THAT MOMENT); `NotCurrentlyPlaced` then lets
//! that entity be re-placed with a DIFFERENT type; `place` never touches
//! `ControlState.edges`, and `remove` only supersedes edges that existed at
//! remove-time — so an edge asserted in the window between a remove and the
//! next re-place is permanently geometry-stale, with no forward move able
//! to revisit it. Minimised reproduction:
//! `fuzz/regressions/board_c2_geometry_determinism/finding_2_geometry_closure_stale_retype`.
//!
//! `connect_refuses_a_withdrawn_endpoint`, both directions (withdrawn as
//! `from`, withdrawn as `to`), through BOTH live call paths —
//! `UboEdgeConnect::execute` (the op) and `KycWorkbook::commit` (the
//! workbook) — same two-surface discipline as
//! `kyc_every_rule_every_surface.rs`'s `PiercedFromIsActiveNominee` gate
//! (audit item 2). At P0 (this file's first commit) both directions on both
//! surfaces ADMIT the connect — pasted as proof of the live defect. P1
//! promotes `Precondition::ConnectEndpointsNotWithdrawn` onto `connect`'s
//! lexicon entry and flips these assertions to expect refusal.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{KycSubjectPlace, KycSubjectRemove, UboEdgeConnect};
use ob_poc::domain_ops::kyc_workbook::open_workbook;
use ob_poc_kyc_substrate::{assembly_lexicon, FoldRegistry, SubjectId, V1FoldImpl};
use ob_poc_types::TransactionScopeId;
use sem_os_core::principal::Principal as RuntimePrincipal;
use sem_os_postgres::ops::SemOsVerbOp;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn connect_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(8)
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
        actor_id: "connect-withdrawn-endpoint-gate".to_string(),
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

/// Which endpoint of the fixture's (voter --voting_rights--> company) edge
/// gets withdrawn — geometry for `voting_rights` is directional
/// (natural_person → company), so both RED-gate directions must hold that
/// direction fixed and vary only which endpoint is withdrawn, or a
/// legitimate `TypeGeometryPermits` refusal would confound the probe.
enum Withdraw {
    Voter,
    Company,
}

/// Places subject + voter (natural_person) + company (private_limited_company),
/// withdraws whichever one `which` names, leaves the other actively placed.
/// Returns (subject, voter, company).
async fn withdrawn_fixture(pool: &PgPool, which: Withdraw) -> (SubjectId, Uuid, Uuid) {
    let subject = SubjectId(Uuid::new_v4());
    let voter = Uuid::new_v4();
    let company = Uuid::new_v4();

    let mut scope = TestScope::begin(pool).await;
    KycSubjectPlace
        .execute(
            &serde_json::json!({ "subject-id": subject.0.to_string(), "entity-type": "private_limited_company" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("place subject");
    KycSubjectPlace
        .execute(
            &serde_json::json!({ "subject-id": subject.0.to_string(), "entity-id": voter.to_string(), "entity-type": "natural_person" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("place voter");
    KycSubjectPlace
        .execute(
            &serde_json::json!({ "subject-id": subject.0.to_string(), "entity-id": company.to_string(), "entity-type": "private_limited_company" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("place company");
    let withdrawn_entity = match which {
        Withdraw::Voter => voter,
        Withdraw::Company => company,
    };
    KycSubjectRemove
        .execute(
            &serde_json::json!({ "subject-id": subject.0.to_string(), "entity-id": withdrawn_entity.to_string() }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("withdraw the fixture's designated entity");
    scope.commit().await;

    (subject, voter, company)
}

/// Direction 1: withdrawn entity as `from_entity_id` (the voter). Through
/// the op. Geometry direction held fixed (voter --voting_rights--> company)
/// so only the withdrawal defect is under test.
#[tokio::test]
async fn op_refuses_withdrawn_from_endpoint() {
    let pool = connect_pool().await;
    let (subject, voter, company) = withdrawn_fixture(&pool, Withdraw::Voter).await;

    let result = {
        let mut scope = TestScope::begin(&pool).await;
        UboEdgeConnect
            .execute(
                &serde_json::json!({
                    "subject-id": subject.0.to_string(),
                    "from_entity_id": voter.to_string(),
                    "to_entity_id": company.to_string(),
                    "kind": "voting_rights",
                }),
                &mut VerbExecutionContext::default(),
                &mut scope,
            )
            .await
    };
    // Cleanup BEFORE the assert — `withdrawn_fixture` always commits real
    // rows regardless of what this probe finds; if the assert below panics
    // (as it must during a RED run), a cleanup call placed after it would
    // never execute (unwind skips it), leaking fixture rows under whatever
    // lexicon_hash was live at RED-proving time — exactly what happened
    // once already in this tranche (P0's RED run left 43 orphaned rows,
    // caught by `kyc_clean_start_gates::fold_registry_has_no_unregistered_hashes_in_the_stream`).
    cleanup(&pool, subject).await;
    assert!(
        result.is_err(),
        "EOP-VS-UBO-GAME-001 §3.4 geometry-closure ruling (2026-09-07): the op must refuse \
         connecting FROM a withdrawn entity {voter} — got: {result:?}"
    );
}

/// Direction 2: withdrawn entity as `to_entity_id` (the company). Through
/// the op. Geometry direction held fixed (voter --voting_rights--> company)
/// so only the withdrawal defect is under test.
#[tokio::test]
async fn op_refuses_withdrawn_to_endpoint() {
    let pool = connect_pool().await;
    let (subject, voter, company) = withdrawn_fixture(&pool, Withdraw::Company).await;

    let result = {
        let mut scope = TestScope::begin(&pool).await;
        UboEdgeConnect
            .execute(
                &serde_json::json!({
                    "subject-id": subject.0.to_string(),
                    "from_entity_id": voter.to_string(),
                    "to_entity_id": company.to_string(),
                    "kind": "voting_rights",
                }),
                &mut VerbExecutionContext::default(),
                &mut scope,
            )
            .await
    };
    cleanup(&pool, subject).await;
    assert!(
        result.is_err(),
        "EOP-VS-UBO-GAME-001 §3.4 geometry-closure ruling (2026-09-07): the op must refuse \
         connecting TO a withdrawn entity {company} — got: {result:?}"
    );
}

/// Direction 1: withdrawn entity as `from_entity_id` (the voter). Through
/// the workbook.
#[tokio::test]
async fn workbook_refuses_withdrawn_from_endpoint() {
    let pool = connect_pool().await;
    let registry = v1_registry();
    let (subject, voter, company) = withdrawn_fixture(&pool, Withdraw::Voter).await;

    let commit_result = {
        let mut scope = TestScope::begin(&pool).await;
        let mut workbook = open_workbook(scope.transaction(), subject).await.unwrap();
        workbook
            .stage(
                &format!(
                    r#"(kyc_ubo.assert.edge.connect :subject-id "{}" :from_entity_id "{voter}" :to_entity_id "{company}" :kind "voting_rights")"#,
                    subject.0
                ),
                &runtime_principal(),
                ob_poc_kyc_substrate::AuthorityRef("gate".into()),
                fixed_ts(),
            )
            .expect("staging recognises (frontier admits any connect)");
        workbook.commit(&mut scope, &registry).await
    };
    cleanup(&pool, subject).await;
    assert!(
        commit_result.is_err(),
        "EOP-VS-UBO-GAME-001 §3.4 geometry-closure ruling (2026-09-07): the workbook must \
         refuse connecting FROM a withdrawn entity {voter} — got: {commit_result:?}"
    );
}

/// Direction 2: withdrawn entity as `to_entity_id` (the company). Through
/// the workbook.
#[tokio::test]
async fn workbook_refuses_withdrawn_to_endpoint() {
    let pool = connect_pool().await;
    let registry = v1_registry();
    let (subject, voter, company) = withdrawn_fixture(&pool, Withdraw::Company).await;

    let commit_result = {
        let mut scope = TestScope::begin(&pool).await;
        let mut workbook = open_workbook(scope.transaction(), subject).await.unwrap();
        workbook
            .stage(
                &format!(
                    r#"(kyc_ubo.assert.edge.connect :subject-id "{}" :from_entity_id "{voter}" :to_entity_id "{company}" :kind "voting_rights")"#,
                    subject.0
                ),
                &runtime_principal(),
                ob_poc_kyc_substrate::AuthorityRef("gate".into()),
                fixed_ts(),
            )
            .expect("staging recognises (frontier admits any connect)");
        workbook.commit(&mut scope, &registry).await
    };
    cleanup(&pool, subject).await;
    assert!(
        commit_result.is_err(),
        "EOP-VS-UBO-GAME-001 §3.4 geometry-closure ruling (2026-09-07): the workbook must \
         refuse connecting TO a withdrawn entity {company} — got: {commit_result:?}"
    );
}
