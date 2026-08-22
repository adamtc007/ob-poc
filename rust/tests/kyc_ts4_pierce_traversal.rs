//! TS.4 gate tests — EOP-DD-KYCUBO-TS.4 §3 Ruling B (piercing moves into
//! the traversal itself, per TS.0 §5 / K-8). Live-DB harness, same pattern
//! as `kyc_ts4_nominee.rs`/`kyc_ts4_fund_pivot_evidence.rs`: drives the REAL
//! `ubo.determination.freeze` op end-to-end across three different
//! top-level strategies.
//!
//! The mechanism itself: an already-pierced nominee's replacement edge is
//! an ordinary `Traverse`-kind edge with the same `to` as the superseded
//! `Nominee` edge, so `reconciled_control_edges` walks straight through it
//! — that has worked since the `pierce-nominee` verb landed (TS.0 §2.6).
//! What Ruling B actually fixes: (a) the freeze-dispatch K-8 guard was
//! scoped to `strategy_name == "nominee_pierce_strategy"` only, so an
//! UNPIERCED nominee sitting mid-chain under ANY OTHER strategy was
//! entirely invisible and freeze would succeed silently; (b) nothing
//! recorded that a pierce was followed to reach a determination. Both are
//! fixed at the op layer / `recover_determination_at`, universally, not
//! inside any one `DeterminationStrategy::resolve()`.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectClassifyStructure, KycSubjectRegister, UboDeterminationFreeze,
    UboDeterminationSelectStrategy, UboEdgeAssertControl, UboEdgeAttachEvidence,
    UboEdgePierceNominee, UboEdgeReconcileConflict,
};
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
        .expect("DB")
}

struct Scope {
    tx: Transaction<'static, Postgres>,
    pool: PgPool,
    id: TransactionScopeId,
}
impl Scope {
    async fn begin(p: &PgPool) -> Self {
        Self { tx: p.begin().await.unwrap(), pool: p.clone(), id: TransactionScopeId::new() }
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

async fn cleanup(pool: &PgPool, subject: Uuid) {
    for t in [
        "kyc_intent_events",
        "kyc_subject_streams",
        "kyc_control_edge_projection",
        "kyc_obligation_projection",
        "kyc_subject_rollup_projection",
    ] {
        let _ =
            sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
                .bind(subject)
                .execute(pool)
                .await;
    }
    let _ = sqlx::query(
        r#"DELETE FROM "public".outbox WHERE idempotency_key LIKE $1
           OR (payload->>'determination_subject')::text = $2
           OR (payload->>'subject_root')::text = $2"#,
    )
    .bind(format!("{subject}:%"))
    .bind(subject.to_string())
    .execute(pool)
    .await;
}

async fn setup_subject(pool: &PgPool, subject: Uuid, natural_persons: &[Uuid], class: &str) {
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject, "is_natural_person": false }),
        pool,
    )
    .await;
    for p in natural_persons {
        run(
            &KycSubjectRegister,
            serde_json::json!({ "subject-id": subject, "entity-id": p, "is_natural_person": true }),
            pool,
        )
        .await;
    }
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({ "subject-id": subject, "structure-class": class }),
        pool,
    )
    .await;
}

// ── Headline: mid_chain_nominee_is_pierced_in_every_strategy ───────────────

#[tokio::test]
async fn mid_chain_nominee_is_pierced_in_every_strategy() {
    let pool = pool().await;

    // (a) FUND — fund_control_strategy. Fund <-ManagementMandate- ManCo
    // <-Nominee- NomineeCorp (unpierced); pierce reveals Alice.
    {
        let subject = Uuid::new_v4();
        let manco = Uuid::new_v4();
        let nominee_corp = Uuid::new_v4();
        let alice = Uuid::new_v4();
        let nominee_edge = Uuid::new_v4();

        setup_subject(&pool, subject, &[alice], "investment_fund").await;
        let mandate_edge = Uuid::new_v4();
        run(
            &UboEdgeAssertControl,
            serde_json::json!({
                "subject-id": subject, "from_entity_id": manco, "to_entity_id": subject,
                "kind": "management_mandate", "edge-id": mandate_edge,
            }),
            &pool,
        )
        .await;
        // Evidenced up front — this test is about the nominee guard (Ruling
        // B), not the evidence stud (Ruling 2f, covered separately in
        // `kyc_ts4_fund_pivot_evidence.rs`).
        run(
            &UboEdgeAttachEvidence,
            serde_json::json!({ "subject-id": subject, "edge-id": mandate_edge }),
            &pool,
        )
        .await;
        run(
            &UboEdgeAssertControl,
            serde_json::json!({
                "subject-id": subject, "from_entity_id": nominee_corp, "to_entity_id": manco,
                "kind": "nominee", "edge-id": nominee_edge,
            }),
            &pool,
        )
        .await;
        run(&UboEdgeReconcileConflict, serde_json::json!({ "subject-id": subject }), &pool).await;
        run(
            &UboDeterminationSelectStrategy,
            serde_json::json!({ "subject-id": subject, "strategy": "fund_control_strategy" }),
            &pool,
        )
        .await;

        let refused = run_fallible(
            &UboDeterminationFreeze,
            serde_json::json!({ "subject-id": subject, "policy-version": "v1.0" }),
            &pool,
        )
        .await;
        assert!(refused.is_err(), "fund: freeze must refuse while the mid-chain nominee is unpierced");
        assert!(refused.unwrap_err().to_string().contains(&nominee_edge.to_string()));

        run(
            &UboEdgePierceNominee,
            serde_json::json!({
                "subject-id": subject, "edge-id": nominee_edge,
                "nominator-id": alice, "kind": "voting_rights",
            }),
            &pool,
        )
        .await;
        let outcome = run(
            &UboDeterminationFreeze,
            serde_json::json!({ "subject-id": subject, "policy-version": "v1.0" }),
            &pool,
        )
        .await;
        let candidates =
            outcome.get("candidates").and_then(|c| c.as_array()).cloned().unwrap_or_default();
        assert_eq!(candidates.len(), 1, "fund: Alice resolves post-pierce: {outcome:?}");
        assert_eq!(
            candidates[0].get("person_id").and_then(|v| v.as_str()),
            Some(alice.to_string()).as_deref(),
        );

        cleanup(&pool, subject).await;
    }

    // (b) CORPORATE — control_prong_strategy. Corp <-VotingRights- HoldCo
    // <-Nominee- NomineeCorp2 (unpierced); pierce reveals Carol.
    {
        let subject = Uuid::new_v4();
        let holdco = Uuid::new_v4();
        let nominee_corp = Uuid::new_v4();
        let carol = Uuid::new_v4();
        let nominee_edge = Uuid::new_v4();

        setup_subject(&pool, subject, &[carol], "private_company").await;
        run(
            &UboEdgeAssertControl,
            serde_json::json!({
                "subject-id": subject, "from_entity_id": holdco, "to_entity_id": subject,
                "kind": "voting_rights",
            }),
            &pool,
        )
        .await;
        run(
            &UboEdgeAssertControl,
            serde_json::json!({
                "subject-id": subject, "from_entity_id": nominee_corp, "to_entity_id": holdco,
                "kind": "nominee", "edge-id": nominee_edge,
            }),
            &pool,
        )
        .await;
        run(&UboEdgeReconcileConflict, serde_json::json!({ "subject-id": subject }), &pool).await;
        run(
            &UboDeterminationSelectStrategy,
            serde_json::json!({ "subject-id": subject, "strategy": "control_prong_strategy" }),
            &pool,
        )
        .await;

        let refused = run_fallible(
            &UboDeterminationFreeze,
            serde_json::json!({ "subject-id": subject, "policy-version": "v1.0" }),
            &pool,
        )
        .await;
        assert!(refused.is_err(), "corporate: freeze must refuse while the mid-chain nominee is unpierced");

        run(
            &UboEdgePierceNominee,
            serde_json::json!({
                "subject-id": subject, "edge-id": nominee_edge,
                "nominator-id": carol, "kind": "voting_rights",
            }),
            &pool,
        )
        .await;
        let outcome = run(
            &UboDeterminationFreeze,
            serde_json::json!({ "subject-id": subject, "policy-version": "v1.0" }),
            &pool,
        )
        .await;
        let candidates =
            outcome.get("candidates").and_then(|c| c.as_array()).cloned().unwrap_or_default();
        assert_eq!(candidates.len(), 1, "corporate: Carol resolves post-pierce: {outcome:?}");
        assert_eq!(
            candidates[0].get("person_id").and_then(|v| v.as_str()),
            Some(carol.to_string()).as_deref(),
        );

        cleanup(&pool, subject).await;
    }

    // (c) TRUST — trust_role_strategy. Trust <-Nominee- NomineeCorp3
    // (unpierced; the trustee SLOT itself is fronted by the nominee — the
    // v1 boundary on `TrustRoleStrategy` admits only qualifying trust-kind
    // edges at every hop, so a nominee arrangement is realistically
    // 1-level here rather than 2 — still classified `trust`, never
    // `nominee`); pierce reveals Dave AS the real trustee.
    {
        let subject = Uuid::new_v4();
        let nominee_corp = Uuid::new_v4();
        let dave = Uuid::new_v4();
        let nominee_edge = Uuid::new_v4();

        setup_subject(&pool, subject, &[dave], "trust").await;
        run(
            &UboEdgeAssertControl,
            serde_json::json!({
                "subject-id": subject, "from_entity_id": nominee_corp, "to_entity_id": subject,
                "kind": "nominee", "edge-id": nominee_edge,
            }),
            &pool,
        )
        .await;
        run(&UboEdgeReconcileConflict, serde_json::json!({ "subject-id": subject }), &pool).await;
        run(
            &UboDeterminationSelectStrategy,
            serde_json::json!({ "subject-id": subject, "strategy": "trust_role_strategy" }),
            &pool,
        )
        .await;

        let refused = run_fallible(
            &UboDeterminationFreeze,
            serde_json::json!({ "subject-id": subject, "policy-version": "v1.0" }),
            &pool,
        )
        .await;
        assert!(refused.is_err(), "trust: freeze must refuse while the trustee-slot nominee is unpierced");

        run(
            &UboEdgePierceNominee,
            serde_json::json!({
                "subject-id": subject, "edge-id": nominee_edge,
                "nominator-id": dave, "kind": "trust_trustee",
            }),
            &pool,
        )
        .await;
        let outcome = run(
            &UboDeterminationFreeze,
            serde_json::json!({ "subject-id": subject, "policy-version": "v1.0" }),
            &pool,
        )
        .await;
        let candidates =
            outcome.get("candidates").and_then(|c| c.as_array()).cloned().unwrap_or_default();
        assert_eq!(candidates.len(), 1, "trust: Dave resolves post-pierce: {outcome:?}");
        assert_eq!(
            candidates[0].get("person_id").and_then(|v| v.as_str()),
            Some(dave.to_string()).as_deref(),
        );

        cleanup(&pool, subject).await;
    }
}

// ── pierce_is_recorded ───────────────────────────────────────────────────

#[tokio::test]
async fn pierce_is_recorded() {
    let pool = pool().await;
    let subject = Uuid::new_v4();
    let manco = Uuid::new_v4();
    let nominee_corp = Uuid::new_v4();
    let alice = Uuid::new_v4();
    let nominee_edge = Uuid::new_v4();

    setup_subject(&pool, subject, &[alice], "investment_fund").await;
    let mandate_edge = Uuid::new_v4();
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject, "from_entity_id": manco, "to_entity_id": subject,
            "kind": "management_mandate", "edge-id": mandate_edge,
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeAttachEvidence,
        serde_json::json!({ "subject-id": subject, "edge-id": mandate_edge }),
        &pool,
    )
    .await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject, "from_entity_id": nominee_corp, "to_entity_id": manco,
            "kind": "nominee", "edge-id": nominee_edge,
        }),
        &pool,
    )
    .await;
    run(&UboEdgeReconcileConflict, serde_json::json!({ "subject-id": subject }), &pool).await;
    run(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject, "strategy": "fund_control_strategy" }),
        &pool,
    )
    .await;
    run(
        &UboEdgePierceNominee,
        serde_json::json!({
            "subject-id": subject, "edge-id": nominee_edge,
            "nominator-id": alice, "kind": "voting_rights",
        }),
        &pool,
    )
    .await;
    // The pierce op's outcome only exposes the SUPERSEDED (target) edge id;
    // the replacement edge id is deterministically derived by the fold
    // (`fold/control.rs`'s `"ubo.edge.pierce-nominee"` arm) as
    // `Uuid::new_v5(NAMESPACE_OID, "control:{nominator}:{to}:{kind:?}")` —
    // replicated here rather than re-deriving it from a second DB read.
    let replacement_edge_id =
        Uuid::new_v5(&Uuid::NAMESPACE_OID, format!("control:{alice}:{manco}:VotingRights").as_bytes())
            .to_string();

    let outcome = run(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    let candidates = outcome.get("candidates").and_then(|c| c.as_array()).cloned().unwrap_or_default();
    assert_eq!(candidates.len(), 1, "{outcome:?}");
    let pierces = candidates[0].get("pierces").and_then(|p| p.as_array()).cloned().unwrap_or_default();
    assert_eq!(pierces.len(), 1, "the pierce followed to reach Alice must be recorded: {outcome:?}");
    assert_eq!(
        pierces[0].get("nominee_entity").and_then(|v| v.as_str()),
        Some(nominee_corp.to_string()).as_deref(),
    );
    assert_eq!(
        pierces[0].get("underlying_holder").and_then(|v| v.as_str()),
        Some(alice.to_string()).as_deref(),
    );
    assert_eq!(
        pierces[0].get("nominee_edge_id").and_then(|v| v.as_str()),
        Some(nominee_edge.to_string()).as_deref(),
    );
    assert_eq!(
        pierces[0].get("replacement_edge_id").and_then(|v| v.as_str()),
        Some(replacement_edge_id).as_deref(),
    );

    cleanup(&pool, subject).await;
}

// ── determination_never_terminates_at_a_nominee ─────────────────────────────

#[tokio::test]
async fn determination_never_terminates_at_a_nominee() {
    // A structure where, pre-Ruling-B, freeze would have "completed"
    // (0 candidates, no error — a silently empty determination) because the
    // unpierced-nominee guard was scoped to nominee_pierce_strategy only.
    // HoldCo dead-ends completely once the Nominee edge is excluded from
    // `reconciled_control_edges` — no officers to pull, nothing to name —
    // yet the arrangement is real. Ruling B: freeze must STILL refuse,
    // under control_prong_strategy, never silently "terminating" here.
    let pool = pool().await;
    let subject = Uuid::new_v4();
    let holdco = Uuid::new_v4();
    let nominee_corp = Uuid::new_v4();
    let nominee_edge = Uuid::new_v4();

    setup_subject(&pool, subject, &[], "private_company").await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject, "from_entity_id": holdco, "to_entity_id": subject,
            "kind": "voting_rights",
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject, "from_entity_id": nominee_corp, "to_entity_id": holdco,
            "kind": "nominee", "edge-id": nominee_edge,
        }),
        &pool,
    )
    .await;
    run(&UboEdgeReconcileConflict, serde_json::json!({ "subject-id": subject }), &pool).await;
    run(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject, "strategy": "control_prong_strategy" }),
        &pool,
    )
    .await;

    let result = run_fallible(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    assert!(
        result.is_err(),
        "freeze must refuse rather than silently 'complete' with zero candidates while an \
         unpierced nominee sits in the graph: {result:?}"
    );
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains(&nominee_edge.to_string()), "the error must name the unpierced edge; got: {msg}");

    cleanup(&pool, subject).await;
}
