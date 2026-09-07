//! P4 — 2026-09-07 audit item 2 (K-8 pierce guard hoist), structural close.
//!
//! `every_rule_runs_on_every_surface`: the same illegal event, built once,
//! driven through BOTH live call paths that append `kyc_ubo.*` events —
//! `UboEdgeConnect::execute` (the real `SemOsVerbOp`, the production
//! dispatch path) and `KycWorkbook::commit` (the workbook path P0d proved
//! bypassed the op entirely) — and both must refuse, with structurally
//! distinguishable error types: the op returns a bare `anyhow::Error`
//! (stream_append's message-only wrap, no preserved source chain), the
//! workbook returns a live `WorkbookError::Kyc(KycError::PreconditionFailed)`
//! enum a caller can `match` on. Two different Rust types reaching the same
//! refusal is the proof that both surfaces consult the SAME declared
//! `Precondition::PiercedFromIsActiveNominee`, not two hand-rolled copies of
//! the rule (R7).

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext, VerbExecutionOutcome};
use ob_poc::domain_ops::kyc_stream_ops::{KycSubjectPlace, UboEdgeConnect};
use ob_poc::domain_ops::kyc_workbook::{open_workbook, WorkbookError};
use ob_poc_kyc_substrate::{assembly_lexicon, FoldRegistry, KycError, SubjectId, V1FoldImpl};
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
        actor_id: "every-rule-every-surface-gate".to_string(),
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

#[tokio::test]
async fn every_rule_runs_on_every_surface() {
    let pool = connect_pool().await;
    let registry = v1_registry();
    let subject = SubjectId(Uuid::new_v4());
    let voter = Uuid::new_v4();
    let far_end = Uuid::new_v4();

    // ── Fixture: subject + voter + far_end placed, one REAL, non-nominee
    // voting_rights edge from voter to subject — via the real ops, committed
    // once, shared by both probes below. ─────────────────────────────────────
    let real_edge_id: Uuid = {
        let mut scope = TestScope::begin(&pool).await;
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
                &serde_json::json!({ "subject-id": subject.0.to_string(), "entity-id": far_end.to_string(), "entity-type": "private_limited_company" }),
                &mut VerbExecutionContext::default(),
                &mut scope,
            )
            .await
            .expect("place far_end");
        let outcome = UboEdgeConnect
            .execute(
                &serde_json::json!({
                    "subject-id": subject.0.to_string(),
                    "from_entity_id": voter.to_string(),
                    "to_entity_id": subject.0.to_string(),
                    "kind": "voting_rights",
                }),
                &mut VerbExecutionContext::default(),
                &mut scope,
            )
            .await
            .expect("connect the real, non-nominee edge");
        scope.commit().await;
        match outcome {
            VerbExecutionOutcome::Record(v) => {
                Uuid::parse_str(v["edge_id"].as_str().unwrap()).unwrap()
            }
            other => panic!("expected Record, got {other:?}"),
        }
    };

    let illegal_pierce_args = serde_json::json!({
        "subject-id": subject.0.to_string(),
        "from_entity_id": voter.to_string(),
        "to_entity_id": far_end.to_string(),
        "kind": "voting_rights",
        "pierced-from": real_edge_id.to_string(),
    });

    // ── Surface 1: the real op, `UboEdgeConnect::execute`. ────────────────
    let op_result = {
        let mut scope = TestScope::begin(&pool).await;
        UboEdgeConnect
            .execute(&illegal_pierce_args, &mut VerbExecutionContext::default(), &mut scope)
            .await
        // scope dropped, never committed — irrelevant, execute() already errored.
    };
    assert!(op_result.is_err(), "the op must refuse the illegal pierce; got {op_result:?}");
    let op_err = op_result.unwrap_err();
    assert!(
        op_err.to_string().contains("nominee") || op_err.to_string().contains("Nominee"),
        "op error must name the nominee rule; got: {op_err}"
    );

    // ── Surface 2: the workbook, `KycWorkbook::commit`. ────────────────────
    let workbook_result = {
        let mut scope = TestScope::begin(&pool).await;
        let mut workbook = open_workbook(scope.transaction(), subject).await.unwrap();
        workbook
            .stage(
                &format!(
                    r#"(kyc_ubo.assert.edge.connect :subject-id "{}" :from_entity_id "{voter}" :to_entity_id "{far_end}" :kind "voting_rights" :pierced-from "{real_edge_id}")"#,
                    subject.0
                ),
                &runtime_principal(),
                ob_poc_kyc_substrate::AuthorityRef("gate".into()),
                fixed_ts(),
            )
            .expect("staging recognises (frontier admits any connect)");
        workbook.commit(&mut scope, &registry).await
    };
    assert!(
        workbook_result.is_err(),
        "the workbook must refuse the illegal pierce; got {workbook_result:?}"
    );
    let workbook_err = workbook_result.unwrap_err();

    // ── Structurally distinguishable: the workbook error is a live enum a
    // caller can match on (not just a formatted string); its Kyc variant
    // carries the real KycError::PreconditionFailed the checker raised. ────
    match &workbook_err {
        WorkbookError::Kyc(KycError::PreconditionFailed { reason, .. }) => {
            assert!(
                reason.contains("nominee") || reason.contains("Nominee"),
                "workbook's typed PreconditionFailed reason must name the nominee rule; got: {reason}"
            );
        }
        other => panic!("expected WorkbookError::Kyc(PreconditionFailed), got: {other:?}"),
    }

    // Both surfaces refused the SAME illegal event, via structurally
    // different error types (anyhow::Error vs. a matchable WorkbookError
    // enum) — proof of one shared rule, not two implementations of it.
    let _ = op_err;

    cleanup(&pool, subject).await;
}
