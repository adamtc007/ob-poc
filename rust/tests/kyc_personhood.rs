//! Personhood-divergence gates — the 2026-08-28 audit's A1b finding, closed
//! by the "entity type is the single source of truth for personhood" ruling
//! (2026-09-07). RED-first: at authoring time `natural_persons_from_events`
//! (`fold/control.rs`) read ONLY the payload flag `is_natural_person`, so a
//! correctly-typed 50% owner placed without the redundant flag was silently
//! absent from a frozen determination of record — and `freeze` has no
//! inverse. The flag was a second vocabulary for one concept, the defect
//! family this programme has closed four times (EdgeKind/Pipe, pack
//! ownership, FQN segments, structure-class/entity-type).
//!
//! - `typed_person_resolves_without_the_flag` — the audit probe as a gate:
//!   two 50% economic holders, both `entity-type: natural_person`, one
//!   carrying the legacy flag and one not. BOTH must resolve. (RED today:
//!   only the flagged one resolves.)
//! - `flag_cannot_contradict_the_type` — the inverse divergence: an entity
//!   typed as a company must NOT become a traversal terminus no matter what
//!   any payload flag claims.
//! - `register_era_flag_is_still_read_where_no_type_exists` — R5: 207
//!   committed register-era events carry ONLY the flag (no entity_type
//!   payload key exists on `register`), so personhood for a type-less
//!   entity still reads the historical flag rather than fabricating
//!   non-personhood on replay. Live `place` always carries a type, so this
//!   arm is unreachable from the current vocabulary.
//! - `personhood_has_one_source` — structural (grep-proof): the live
//!   vocabulary no longer declares, shapes, or copies `is_natural_person`
//!   anywhere — YAML arg gone, canonical `place` arm copies nothing,
//!   and the fold's only flag read sits inside the documented
//!   R5-historical no-type fallback.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{KycSubjectPlace, UboDeterminationFreeze, UboEdgeConnect};
use ob_poc_kyc_substrate::{
    natural_persons_from_events, AuthorityRef, IntentEvent, PersonId, Principal, SubjectId,
    TargetBinding, VerbFqn,
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

async fn run_ok(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> serde_json::Value {
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

async fn cleanup(pool: &PgPool, s: SubjectId) {
    for t in ["kyc_intent_events", "kyc_subject_streams", "kyc_control_edge_projection"] {
        let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
            .bind(s.0)
            .execute(pool)
            .await;
    }
    let _ = sqlx::query(r#"DELETE FROM "public".outbox WHERE payload::text LIKE $1"#)
        .bind(format!("%{}%", s.0))
        .execute(pool)
        .await;
}

fn place_args(s: SubjectId, e: Uuid, ty: &str) -> serde_json::Value {
    serde_json::json!({
        "subject-id": s.0.to_string(),
        "entity-id": e.to_string(),
        "entity-type": ty,
    })
}

fn economic_edge(s: SubjectId, from: Uuid, to: Uuid, pct: f64) -> serde_json::Value {
    serde_json::json!({
        "subject-id": s.0.to_string(),
        "from_entity_id": from.to_string(),
        "to_entity_id": to.to_string(),
        "kind": "economic_interest",
        "percentage": pct,
    })
}

/// The audit's A1b probe as a permanent gate. Two 50% economic holders, both
/// placed `entity-type: natural_person`; one ALSO carries the legacy flag
/// (which the live path must now ignore — it is not a declared arg any
/// longer, so it simply falls off), one does not. Both must resolve as
/// candidates of the frozen determination. RED at authoring time: the
/// flag-only terminus resolved exactly one.
#[tokio::test]
async fn typed_person_resolves_without_the_flag() {
    let pool = pool().await;
    let s = SubjectId(Uuid::new_v4());
    let p1 = Uuid::new_v4();
    let p2 = Uuid::new_v4();

    run_ok(&KycSubjectPlace, place_args(s, s.0, "private_limited_company"), &pool).await;
    // p1: typed person + the legacy flag riding along in args (must be inert)
    let mut a1 = place_args(s, p1, "natural_person");
    a1["is_natural_person"] = serde_json::json!(true);
    run_ok(&KycSubjectPlace, a1, &pool).await;
    // p2: typed person, no flag — the audit's silently-dropped owner
    run_ok(&KycSubjectPlace, place_args(s, p2, "natural_person"), &pool).await;

    run_ok(&UboEdgeConnect, economic_edge(s, p1, s.0, 50.0), &pool).await;
    run_ok(&UboEdgeConnect, economic_edge(s, p2, s.0, 50.0), &pool).await;

    let det = run_ok(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": s.0.to_string(), "policy-version": "kyc-personhood-gate" }),
        &pool,
    )
    .await;
    let candidates: Vec<String> = det["candidates"]
        .as_array()
        .expect("freeze returns candidates")
        .iter()
        .map(|c| c["person_id"].as_str().unwrap_or("?").to_string())
        .collect();

    let p1_present = candidates.iter().any(|c| c == &p1.to_string());
    let p2_present = candidates.iter().any(|c| c == &p2.to_string());
    cleanup(&pool, s).await;

    assert!(
        p1_present && p2_present,
        "both correctly-typed 50% owners must resolve as candidates; got p1={p1_present} \
         p2={p2_present} (candidates: {candidates:?}) — a typed owner absent from a \
         determination of record is the audit's A1b defect"
    );
}

/// The inverse divergence: personhood comes from the type, so an entity
/// typed as a company is NEVER a traversal terminus — even when a legacy
/// flag claims otherwise (pure fold check; the live arg is gone, so the
/// claim can only arrive on a hand-built payload, same as any historical
/// garbage the fold must not trust over a real type assertion).
#[test]
fn flag_cannot_contradict_the_type() {
    let s = SubjectId(Uuid::new_v4());
    let company = Uuid::new_v4();

    let place = IntentEvent::new(
        s,
        VerbFqn("kyc_ubo.assert.subject.place".to_string()),
        Principal::test_analyst(),
        AuthorityRef("gate".into()),
        TargetBinding::for_subject(s),
        serde_json::json!({
            "entity_id": company.to_string(),
            "entity_type": "private_limited_company",
            // adversarial: the retired flag contradicting the type
            "is_natural_person": true,
        }),
        chrono::Utc::now(),
    );
    let events = [&place];
    let persons = natural_persons_from_events(&events);
    assert!(
        !persons.contains(&PersonId(company)),
        "an entity typed private_limited_company must not be a natural person, \
         whatever a payload flag claims (the type is the single source of truth)"
    );
}

/// R5 — the historical arm reads what is there. 207 committed register-era
/// events (`kyc.subject.register` / `kyc_ubo.assert.subject.register`) carry
/// ONLY the flag; no entity_type payload key ever existed on `register`.
/// Personhood for an entity with NO type assertion anywhere in the stream
/// still reads the historical flag — dropping it would fabricate
/// non-personhood on replay. Unreachable from the live vocabulary: `place`
/// always carries a required, validated entity-type.
#[test]
fn register_era_flag_is_still_read_where_no_type_exists() {
    let s = SubjectId(Uuid::new_v4());
    let person = Uuid::new_v4();

    let register = IntentEvent::new(
        s,
        VerbFqn("kyc_ubo.assert.subject.register".to_string()),
        Principal::test_analyst(),
        AuthorityRef("gate".into()),
        TargetBinding::for_subject(s),
        serde_json::json!({ "entity_id": person.to_string(), "is_natural_person": true }),
        chrono::Utc::now(),
    );
    let events = [&register];
    let persons = natural_persons_from_events(&events);
    assert!(
        persons.contains(&PersonId(person)),
        "a register-era entity with no type assertion and a true historical flag \
         must remain a natural person on replay (R5: nothing is fabricated)"
    );
}

/// Structural, grep-proof: the live vocabulary has exactly one source of
/// personhood — the entity type. The flag survives only as (a) the fold's
/// documented R5-historical fallback for type-less entities and (b) the
/// historical-payload fixtures that exercise it.
#[test]
fn personhood_has_one_source() {
    // Comments may (and do) mention the retired flag as retirement notes —
    // the codebase convention for every retired vocabulary item. The gates
    // below therefore grep CODE lines only.
    fn code_lines<'a>(src: &'a str, comment_prefix: &str) -> impl Iterator<Item = &'a str> {
        let prefix = comment_prefix.to_string();
        src.lines().filter(move |l| !l.trim_start().starts_with(prefix.as_str()))
    }

    // 1. The YAML no longer declares the arg on any verb.
    let yaml = include_str!("../config/verbs/kyc/dsl-kyc.yaml");
    assert!(
        !code_lines(yaml, "#").any(|l| l.contains("is_natural_person")),
        "config/verbs/kyc/dsl-kyc.yaml still declares is_natural_person — the live \
         vocabulary must carry one personhood channel (the entity type)"
    );

    // 2. The canonical event shape no longer reads or copies it.
    let canonical = include_str!("../crates/ob-poc-kyc-seam/src/canonical.rs");
    assert!(
        !code_lines(canonical, "//").any(|l| l.contains("is_natural_person")),
        "canonical_event_shape still handles is_natural_person — the seam must not \
         carry a second personhood vocabulary"
    );

    // 3. The fold's only reads of the flag sit inside the R5-historical
    //    fallback of `natural_persons_from_events` — behaviourally pinned by
    //    the two tests above (type wins where a type exists; the flag is
    //    read only where NO type assertion exists), and bounded here by
    //    count so a new reader cannot slip in unnoticed.
    let fold = include_str!("../crates/ob-poc-kyc-substrate/src/fold/control.rs");
    let reads = fold.matches("\"is_natural_person\"").count();
    assert_eq!(
        reads, 1,
        "fold/control.rs must read the historical flag in exactly one place \
         (natural_persons_from_events's R5 no-type fallback); found {reads}"
    );
}
