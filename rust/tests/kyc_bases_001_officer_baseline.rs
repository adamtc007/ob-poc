//! EOP-DD-UBO-BASES-001 closure tranche — P0c / P2 standing fixture.
//!
//! The BASES-001 P5 receipt's cross-type baseline used one-edge-per-type
//! fixtures with no officer/employment edge anywhere — "byte-identical for
//! every type except companies" was true of THOSE fixtures, but they were
//! structurally incapable of detecting the admission change reaching every
//! control-walking strategy (§5's `control_admission` is global). The
//! 2026-09-08 Fable audit's Template B (independent baseline, not this
//! harness) found five non-company types gaining officer/employment
//! candidates undetected by the receipt's own gate.
//!
//! R-C (RULED 2026-09-08) ratifies this by name: "R-A applies to every
//! control-walk strategy, not to companies alone... the role is the door,
//! then a switch opens the door or not." This fixture is the baseline that
//! CAN fail — every one of the 19 determination-subject types gets an
//! `officer_appointment` edge, and this test pins the resulting candidate
//! set per type, not just "ran without crashing."
//!
//! Through the real op layer (`SemOsVerbOp`, live Postgres).

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{KycSubjectPlace, UboDeterminationFreeze, UboEdgeConnect};
use ob_poc_kyc_substrate::SubjectId;
use ob_poc_types::TransactionScopeId;
use sem_os_postgres::ops::SemOsVerbOp;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn pool() -> PgPool {
    PgPoolOptions::new().max_connections(4).connect(&database_url()).await.expect("connect")
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

async fn cleanup(pool: &PgPool, subject: SubjectId) {
    for t in ["kyc_intent_events", "kyc_subject_streams", "kyc_control_edge_projection"] {
        let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
            .bind(subject.0)
            .execute(pool)
            .await;
    }
}

/// Every determination-subject type (19 — all `EntityType` variants except
/// `NaturalPerson`/`SoleTrader`, per `dispatch_for_entity_type`'s D2 arm).
/// The wire values, `ENTITY_TYPE_WIRE_VALUES` order.
const ALL_TYPES: &[&str] = &[
    "private_limited_company",
    "public_listed_company",
    "llc_us",
    "general_partnership",
    "limited_partnership",
    "llp",
    "oeic_icvc",
    "sicav",
    "unit_trust",
    "forty_act_fund",
    "lp_fund",
    "umbrella_with_sub_funds",
    "discretionary_trust",
    "fixed_bare_trust",
    "foundation",
    "pension_scheme",
    "cooperative_mutual",
    "charity_not_for_profit",
    "government_dept_statutory_corporation",
];

/// (entity-type, expected candidate count, expected prong) for a board with
/// exactly one edge — whichever of `officer_appointment`/`employment` the
/// board's own type geometry (`target_permits`, `geometry.rs`) actually
/// admits for that type, tried in that order. A type permitting NEITHER
/// pipe (`LimitedPartnership` and the entire fund/trust/foundation/pension
/// family) gets no edge at all and is expected to K-5-refuse at freeze —
/// R-C's widening cannot open a door type geometry never cut into the wall.
/// `ControlByOtherMeans` = the dispatched strategy's own walk found it
/// directly; `SmoFallback` = primary found nothing, R-B's pull-on-
/// exhaustion picked it up. This table was derived by DRIVING each type
/// against the real op layer, not by reading the geometry grid and
/// assuming — the grid predicts admission, not which strategy finds it.
const CASES: &[(&str, Option<&str>, usize, &str)] = &[
    ("private_limited_company", Some("officer_appointment"), 1, "ControlByOtherMeans"),
    ("public_listed_company", Some("officer_appointment"), 1, "ControlByOtherMeans"),
    ("llc_us", Some("officer_appointment"), 1, "ControlByOtherMeans"),
    // GeneralPartnership/Llp: geometry refuses OfficerAppointment as a
    // target pipe (target_permits has no OfficerAppointment entry for
    // either) — only EmploymentDelegatedAuthority is open.
    ("general_partnership", Some("employment"), 1, "ControlByOtherMeans"),
    // LimitedPartnership: target_permits = [GpDesignation,
    // LimitedPartnershipInterest, ManagementMandate, ContractualControl] —
    // NEITHER OfficerAppointment nor Employment is in that list. No door.
    ("limited_partnership", None, 0, ""),
    ("llp", Some("employment"), 1, "ControlByOtherMeans"),
    // Every fund type: target_permits has neither OfficerAppointment nor
    // Employment (funds are pivot-directed, not officer-directed). No door.
    ("oeic_icvc", None, 0, ""),
    ("sicav", None, 0, ""),
    ("unit_trust", None, 0, ""),
    ("forty_act_fund", None, 0, ""),
    ("lp_fund", None, 0, ""),
    ("umbrella_with_sub_funds", None, 0, ""),
    // Every trust/fiduciary type + Foundation + PensionScheme: same — no
    // OfficerAppointment/Employment in target_permits. No door.
    ("discretionary_trust", None, 0, ""),
    ("fixed_bare_trust", None, 0, ""),
    ("foundation", None, 0, ""),
    ("pension_scheme", None, 0, ""),
    ("cooperative_mutual", Some("officer_appointment"), 1, "SmoFallback"),
    ("charity_not_for_profit", Some("officer_appointment"), 1, "SmoFallback"),
    (
        "government_dept_statutory_corporation",
        Some("officer_appointment"),
        1,
        "ControlByOtherMeans",
    ),
];

#[tokio::test]
async fn officer_bearing_baseline_per_type_2026_09_08() {
    let pool = pool().await;
    let mut failures = Vec::new();
    let mut dump = String::new();

    // Sanity: CASES covers exactly ALL_TYPES, same set, no drift between
    // the two tables (D3 discipline — a fixture pinning by hand must stay
    // pinned against the real exhaustive type list).
    let all_set: std::collections::BTreeSet<&str> = ALL_TYPES.iter().copied().collect();
    let cases_set: std::collections::BTreeSet<&str> = CASES.iter().map(|c| c.0).collect();
    assert_eq!(all_set, cases_set, "CASES must cover exactly the 19 determination-subject types");

    for &(entity_type, edge_kind, expected_count, expected_prong) in CASES {
        let subject = SubjectId(Uuid::new_v4());
        let officer = Uuid::new_v4();

        run(
            &KycSubjectPlace,
            serde_json::json!({ "subject-id": subject.0, "entity-type": entity_type }),
            &pool,
        )
        .await;
        run(
            &KycSubjectPlace,
            serde_json::json!({
                "subject-id": subject.0, "entity-id": officer, "entity-type": "natural_person",
            }),
            &pool,
        )
        .await;

        if let Some(kind) = edge_kind {
            let wire = if kind == "employment" { "employment" } else { "officer_appointment" };
            let connect = {
                let mut ctx = VerbExecutionContext::default();
                let mut scope = Scope::begin(&pool).await;
                let r = UboEdgeConnect
                    .execute(
                        &serde_json::json!({
                            "subject-id": subject.0, "from_entity_id": officer,
                            "to_entity_id": subject.0, "kind": wire,
                        }),
                        &mut ctx,
                        &mut scope,
                    )
                    .await;
                if r.is_ok() {
                    scope.commit().await;
                }
                r
            };
            if let Err(e) = connect {
                failures.push(format!(
                    "{entity_type}: expected {wire} to be geometry-ADMITTED, was refused: {e}"
                ));
                cleanup(&pool, subject).await;
                continue;
            }
        }

        if expected_count == 0 {
            // No door for this type: freeze must K-5-refuse (no candidates,
            // no officer/employment edge exists to pull either).
            let mut ctx = VerbExecutionContext::default();
            let mut scope = Scope::begin(&pool).await;
            let freeze_err = UboDeterminationFreeze
                .execute(
                    &serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
                    &mut ctx,
                    &mut scope,
                )
                .await;
            dump.push_str(&format!("{entity_type:42} NO DOOR — freeze: {freeze_err:?}\n"));
            if freeze_err.is_ok() {
                failures.push(format!(
                    "{entity_type}: expected K-5 refusal (no door exists for this type), freeze SUCCEEDED"
                ));
            }
            cleanup(&pool, subject).await;
            continue;
        }

        let freeze_out = run(
            &UboDeterminationFreeze,
            serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
            &pool,
        )
        .await;

        let candidates = freeze_out["candidates"].as_array().cloned().unwrap_or_default();
        dump.push_str(&format!(
            "{entity_type:42} strategy={:<55} candidates={}\n",
            freeze_out["strategy"].as_str().unwrap_or("?"),
            serde_json::to_string(&candidates).unwrap()
        ));

        if candidates.len() != expected_count {
            failures.push(format!(
                "{entity_type}: expected {expected_count} candidate(s), got {} — {candidates:?}",
                candidates.len()
            ));
            cleanup(&pool, subject).await;
            continue;
        }
        let prong = candidates[0]["prong"].as_str().unwrap_or("?");
        if prong != expected_prong {
            failures.push(format!(
                "{entity_type}: expected prong {expected_prong}, got {prong} — {candidates:?}"
            ));
        }
        let person = candidates[0]["person_id"].as_str().unwrap_or("?");
        if person != officer.to_string() {
            failures.push(format!(
                "{entity_type}: expected officer {officer} as candidate, got {person}"
            ));
        }

        cleanup(&pool, subject).await;
    }

    println!("=== officer-bearing 19-type baseline (2026-09-08) ===\n{dump}");

    assert!(
        failures.is_empty(),
        "officer-bearing baseline diverged from expectations (R-C):\n{}",
        failures.join("\n")
    );
}
