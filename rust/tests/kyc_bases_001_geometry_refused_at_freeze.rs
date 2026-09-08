//! EOP-DD-UBO-BASES-001 closure tranche — P0a/P1 standing gate.
//!
//! Reproduction of the 2026-09-08 Fable audit's C3 finding, through the
//! REAL op layer (`SemOsVerbOp::execute`, real Postgres, real
//! `stream_append`/`check_preconditions` chokepoint) — not the substrate
//! `check_preconditions`+fold the audit drove directly.
//!
//! Sequence: connect a `trust_trustee` edge FROM a placed trustee INTO a
//! subject root that is registered (the stream exists — a sibling entity
//! was placed under the same `subject-id`) but NOT YET TYPED — admitted,
//! `TypeGeometryPermits` sees an untyped `to` and is `Unevaluable` (R5/R6,
//! CTN-2e: admits). Then first-place the subject root itself as
//! `private_limited_company` — admitted (`NotCurrentlyPlaced` only refuses
//! an entity already in `registered_entity_ids`, and the root wasn't).
//! Then freeze: the company's new control limb (EOP-DD-UBO-BASES-001 §3)
//! walks the trust_trustee edge as the trustee's sole basis — a triple the
//! ratified geometry matrix (`target_permits(PrivateLimitedCompany)`) has
//! no `TrusteePowers` pipe for.
//!
//! Before P1's fix: `assurance == { reasons: [], .. }` — clean. After: a
//! `ProvisionalityReason::GeometryRefused` names the contradiction, and the
//! freeze still SUCCEEDS (R-C: "the determination names every door with
//! its proof status; it does not decide which open" — name, don't refuse).

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectPlace, UboDeterminationFreeze, UboEdgeAttachEvidence, UboEdgeConnect,
};
use ob_poc_kyc_substrate::SubjectId;
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

/// Same op call but returns the error string on failure instead of panicking
/// — used for the two admission checks this probe must show SUCCEED.
async fn try_run(
    op: &dyn SemOsVerbOp,
    args: serde_json::Value,
    pool: &PgPool,
) -> Result<serde_json::Value, String> {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    match op.execute(&args, &mut ctx, &mut scope).await {
        Ok(out) => {
            scope.commit().await;
            Ok(match out {
                dsl_runtime::VerbExecutionOutcome::Record(v) => v,
                other => serde_json::to_value(format!("{other:?}")).unwrap(),
            })
        }
        Err(e) => Err(e.to_string()),
    }
}

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in ["kyc_intent_events", "kyc_subject_streams", "kyc_control_edge_projection"] {
            let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
                .bind(s.0)
                .execute(pool)
                .await;
        }
        let _ = sqlx::query(
            r#"DELETE FROM "public".outbox WHERE idempotency_key LIKE $1
               OR (payload->>'determination_subject')::text = $2
               OR (payload->>'subject_root')::text = $2"#,
        )
        .bind(format!("{}:%", s.0))
        .bind(s.0.to_string())
        .execute(pool)
        .await;
    }
}

#[tokio::test]
async fn geometry_refused_basis_is_named_not_clean() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4()); // subject root == its own entity id
    let trustee = Uuid::new_v4();

    // Step 1: register the STREAM without typing the root — place a
    // DIFFERENT entity (the trustee, natural_person) under the same
    // subject-id. `control.registered` is stream-level; `type_registry`
    // types only the given `entity_id`. The subject root remains untyped.
    run(
        &KycSubjectPlace,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": trustee, "entity-type": "natural_person",
        }),
        &pool,
    )
    .await;

    // Step 2: connect trust_trustee FROM trustee TO the still-unplaced
    // subject root. Must SUCCEED — `TypeGeometryPermits` sees `to`
    // untyped => `Unevaluable`, which admits (R5/R6, CTN-2e).
    let connect_result = try_run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": trustee, "to_entity_id": subject.0,
            "kind": "trust_trustee",
        }),
        &pool,
    )
    .await;
    println!("=== P0a step 2: connect-before-place (trust_trustee into unplaced root) ===");
    println!("{connect_result:?}");
    assert!(
        connect_result.is_ok(),
        "connect into an unplaced (untyped) subject root must be ADMITTED (Unevaluable, R5/R6) \
         — got: {connect_result:?}"
    );
    let edge_id = connect_result.unwrap()["edge_id"].as_str().unwrap().to_string();

    // Step 3: first-place the subject root as private_limited_company.
    // Must SUCCEED — `NotCurrentlyPlaced` only refuses an entity already in
    // `registered_entity_ids`; the root was never placed, only registered
    // via the trustee's `place` call above.
    let place_result = try_run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;
    println!("=== P0a step 3: first-place subject root as private_limited_company ===");
    println!("{place_result:?}");
    assert!(
        place_result.is_ok(),
        "first-place of the (now edge-touched) subject root must be ADMITTED — got: {place_result:?}"
    );

    // Step 3.5: cite the edge and the subject root's type assertion — the
    // audit's C3b was specifically about a CLEAN assurance (citations
    // logged, not the separate & pre-existing UncitedEdge/UncitedType
    // signal). Without this, `reasons` is non-empty for the ordinary
    // uncited-fact reason, which is not the hole under test.
    run(
        &UboEdgeAttachEvidence,
        serde_json::json!({
            "subject-id": subject.0, "edge-id": edge_id,
            "kind": "constitutional-document", "source": "trust deed", "date": "2026-09-01",
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeAttachEvidence,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": subject.0.to_string(),
            "kind": "registry-extract", "source": "companies house", "date": "2026-09-01",
        }),
        &pool,
    )
    .await;

    // Step 4: freeze. The company's control limb (EOP-DD-UBO-BASES-001 §3)
    // walks the trust_trustee edge; the trustee is the sole candidate.
    let freeze_out = run(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    println!("=== P0a step 4: freeze — RAW OUTPUT ===");
    println!("{}", serde_json::to_string_pretty(&freeze_out).unwrap());

    let candidates = freeze_out["candidates"].as_array().expect("candidates array");
    assert_eq!(candidates.len(), 1, "trustee must be the sole candidate; got {candidates:?}");
    assert_eq!(
        candidates[0]["person_id"].as_str().unwrap(),
        trustee.to_string(),
        "the trustee must be the candidate named"
    );

    let assurance = &freeze_out["assurance"];
    let reasons = assurance["reasons"].as_array().expect("reasons array");
    println!("=== assurance.reasons ===");
    println!("{}", serde_json::to_string_pretty(reasons).unwrap());

    // The freeze must SUCCEED (it did — we're past `run()`'s panic-on-Err)
    // and must NAME the contradiction: a GeometryRefused reason citing the
    // trustee edge's kind, distinct from GeometryUnevaluable (which would
    // mean "could not check," not "checked and forbidden").
    let has_geometry_refused = reasons.iter().any(|r| {
        r.get("GeometryRefused")
            .and_then(|g| g.get("edge_kind_label"))
            .and_then(|k| k.as_str())
            .map(|k| k.contains("Trustee"))
            .unwrap_or(false)
    });
    let has_unevaluable = reasons.iter().any(|r| r.get("GeometryUnevaluable").is_some());
    let failure_dump = freeze_out.clone();
    cleanup(&pool, &[subject]).await;
    assert!(
        has_geometry_refused,
        "P1: freeze on a geometry-forbidden basis edge must carry a named GeometryRefused \
         reason citing the TrustRole(Trustee) edge — assurance: {failure_dump:#}"
    );
    assert!(
        !has_unevaluable,
        "the edge is fully typed both ends with a certain pipe classification (TrustRole \
         always classifies) — GeometryUnevaluable would be the WRONG reason here, that's for \
         'could not check', not 'checked and forbidden': {failure_dump:#}"
    );
}
