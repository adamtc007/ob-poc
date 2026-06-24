//! Phase 1 Task 2 (§10.6) — optimistic-concurrency (CAS) on the directly-mutated
//! compliance write-paths, with the three-class carve-out.
//!
//! v0.5 §10.6 turns LWW into compare-and-set for operator-authored compliance
//! values. The ratified design carves the surface into three classes:
//!   * Class 1 — operator-authored, NON-ManCo compliance facts → version-CAS'd
//!               (a stale concurrent write is REJECTED as a conflict, not clobbered).
//!   * Class 2 — the ManCo designation (A1 role ∈ {MANAGEMENT_COMPANY,
//!               INVESTMENT_MANAGER}; A7 relationship_type='management') →
//!               EXCLUDED from CAS (maker-checker / KYC-canonical authority, not a
//!               symmetric race). A stale write must NOT be version-blocked.
//!   * Class 3 — `ubo_graph` recompute (A7 source LIKE 'ubo.%') → EXCLUDED (derived
//!               plane). A stale write must NOT be version-blocked.
//!
//! This asserts the GREEN side of each: Class-1 stale write rejected; both
//! carve-outs NOT blocked; and the M-13 idempotency floor preserved (a
//! version-LESS re-write still succeeds — CAS and idempotency stack, §10.1).
//!
//! PRIVILEGE CONTEXT (for the receipt):
//!   * Connection: `DATABASE_URL` → role `adamtc007` (PostgreSQL SUPERUSER).
//!   * RLS DISABLED on `cbu_entity_roles` / `entity_relationships` (verified).
//!   * CAS conflict-detection is PRIVILEGE-INVARIANT: the rejection is produced by
//!     the `WHERE version = COALESCE($expected, version)` predicate inside the
//!     UPDATE itself — a 0-row update returns no RETURNING row regardless of the
//!     connection's role. A superuser cannot bypass a row-not-updated outcome;
//!     there is no RLS policy involved in the version comparison. So the superuser
//!     connection cannot manufacture a false-green for a CAS rejection.
//!   * All fixtures live in a `PgTransactionScope` dropped without commit → rolled
//!     back; the test persists nothing.
//!
//! Run:
//!   DATABASE_URL="postgresql:///data_designer" \
//!     cargo test --features database --test cbu_cas_concurrency -- --ignored --nocapture

#![cfg(feature = "database")]

use std::sync::Arc;

use serde_json::json;
use sqlx::types::BigDecimal;
use sqlx::PgPool;
use uuid::Uuid;

use dsl_runtime::{
    SemOsChildDispatcher, ServiceRegistryBuilder, TransactionScope, VerbExecutionContext,
};
use ob_poc::sequencer_tx::PgTransactionScope;
use sem_os_core::principal::Principal;
use sem_os_postgres::ops::cbu_role::{AssignFundRole, AssignOwnership};
use sem_os_postgres::ops::entity_relationship::Upsert as EdgeUpsert;
use sem_os_postgres::ops::{build_registry, RegistryChildDispatcher, SemOsVerbOp};

async fn insert_cbu(scope: &mut dyn TransactionScope, name: &str) -> Uuid {
    sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".cbus (name, jurisdiction) VALUES ($1, 'LU') RETURNING cbu_id"#,
    )
    .bind(name)
    .fetch_one(scope.executor())
    .await
    .expect("cbu inserted")
}

async fn insert_entity(scope: &mut dyn TransactionScope, type_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".entities (entity_type_id, name) VALUES ($1, $2) RETURNING entity_id"#,
    )
    .bind(type_id)
    .bind(name)
    .fetch_one(scope.executor())
    .await
    .expect("entity inserted")
}

async fn any_entity_type(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(r#"SELECT entity_type_id FROM "ob-poc".entity_types LIMIT 1"#)
        .fetch_one(pool)
        .await
        .expect("an entity_type exists")
}

async fn role_version(scope: &mut dyn TransactionScope, cbu_id: Uuid) -> i64 {
    sqlx::query_scalar(r#"SELECT version FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .fetch_one(scope.executor())
        .await
        .expect("role version")
}

async fn role_pct(scope: &mut dyn TransactionScope, cbu_id: Uuid) -> Option<BigDecimal> {
    sqlx::query_scalar(
        r#"SELECT ownership_percentage FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#,
    )
    .bind(cbu_id)
    .fetch_one(scope.executor())
    .await
    .expect("role pct")
}

async fn edge_version(scope: &mut dyn TransactionScope, from: Uuid, to: Uuid, rtype: &str) -> i64 {
    sqlx::query_scalar(
        r#"SELECT version FROM "ob-poc".entity_relationships
           WHERE from_entity_id = $1 AND to_entity_id = $2 AND relationship_type = $3"#,
    )
    .bind(from)
    .bind(to)
    .bind(rtype)
    .fetch_one(scope.executor())
    .await
    .expect("edge version")
}

fn services() -> Arc<dsl_runtime::ServiceRegistry> {
    let mut registry = build_registry();
    ob_poc::domain_ops::extend_registry(&mut registry);
    let dispatcher = Arc::new(RegistryChildDispatcher::new(Arc::new(registry)));
    let mut sb = ServiceRegistryBuilder::new();
    sb.register::<dyn SemOsChildDispatcher>(dispatcher);
    Arc::new(sb.build())
}

/// Class 1 (A1) — a stale concurrent operator write to `ownership_percentage` is
/// REJECTED, not clobbered; and the M-13 version-less floor still succeeds.
#[tokio::test]
#[ignore = "Phase 1 §10.6 CAS: requires DATABASE_URL + database feature"]
async fn cas_a1_rejects_stale_class1_and_keeps_idempotent_floor() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = PgPool::connect(&url).await.expect("connect");
    let (user, is_super): (String, bool) = sqlx::query_as(
        "select current_user, coalesce((select rolsuper from pg_roles where rolname = current_user), false)",
    )
    .fetch_one(&pool).await.expect("priv check");
    eprintln!("[privilege] connected as '{user}', superuser={is_super}; RLS disabled on targets; CAS rejection is privilege-invariant (WHERE-predicate row-not-updated).");
    let type_id = any_entity_type(&pool).await;
    let svc = services();

    let mut scope = PgTransactionScope::begin(&pool).await.expect("begin");
    let mut ctx = VerbExecutionContext::with_services(Principal::system(), svc);
    let cbu = insert_cbu(&mut scope, "cas-a1-class1").await;
    let owner = insert_entity(&mut scope, type_id, "cas-a1-owner").await;
    let owned = insert_entity(&mut scope, type_id, "cas-a1-owned").await;

    let base = |pct: &str, ev: Option<i64>| {
        let mut a = json!({ "cbu-id": cbu, "owner-entity-id": owner, "owned-entity-id": owned,
                            "role": "SHAREHOLDER", "percentage": pct });
        if let Some(v) = ev {
            a["expected-version"] = json!(v);
        }
        a
    };

    // Initial insert → version 1.
    AssignOwnership
        .execute(&base("42.50", None), &mut ctx, &mut scope)
        .await
        .expect("insert");
    assert_eq!(
        role_version(&mut scope, cbu).await,
        1,
        "fresh insert is version 1"
    );

    // Reader-at-v1 commits first: expected-version=1 matches → succeeds, version→2.
    AssignOwnership
        .execute(&base("50.00", Some(1)), &mut ctx, &mut scope)
        .await
        .expect("v1→v2 update");
    assert_eq!(
        role_version(&mut scope, cbu).await,
        2,
        "matched-version update bumps to 2"
    );
    assert_eq!(
        role_pct(&mut scope, cbu).await,
        Some("50.00".parse().unwrap())
    );

    // Stale writer still holding expected-version=1 → REJECTED (version is now 2).
    let stale = AssignOwnership
        .execute(&base("99.99", Some(1)), &mut ctx, &mut scope)
        .await;
    assert!(
        stale.is_err(),
        "stale Class-1 write MUST be rejected, got {stale:?}"
    );
    let msg = format!("{}", stale.unwrap_err());
    assert!(
        msg.contains("CAS conflict"),
        "expected a CAS conflict error, got: {msg}"
    );
    // NOT clobbered: value + version unchanged by the rejected write.
    assert_eq!(
        role_pct(&mut scope, cbu).await,
        Some("50.00".parse().unwrap()),
        "rejected write must not clobber"
    );
    assert_eq!(
        role_version(&mut scope, cbu).await,
        2,
        "rejected write must not bump version"
    );
    eprintln!("[CAS a1] class-1 stale write rejected; value 50.00 / version 2 intact (not clobbered to 99.99)");

    // M-13 floor (CAS and idempotency stack, §10.1): a version-LESS re-write still
    // succeeds — the idempotent upsert is preserved for callers that do not assert
    // a version.
    AssignOwnership
        .execute(&base("55.00", None), &mut ctx, &mut scope)
        .await
        .expect("version-less floor write");
    assert_eq!(
        role_version(&mut scope, cbu).await,
        3,
        "version-less write bumps and is not blocked"
    );
    eprintln!("[CAS a1] M-13 floor intact: version-less re-write succeeded (v3)");
    drop(scope);
}

/// Class 2 (A1) — the ManCo designation BYPASSES CAS: a write carrying a stale
/// (wrong) version is NOT version-blocked.
#[tokio::test]
#[ignore = "Phase 1 §10.6 CAS: requires DATABASE_URL + database feature"]
async fn cas_a1_carveout_manco_not_version_blocked() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = PgPool::connect(&url).await.expect("connect");
    let type_id = any_entity_type(&pool).await;
    let svc = services();
    let mut scope = PgTransactionScope::begin(&pool).await.expect("begin");
    let mut ctx = VerbExecutionContext::with_services(Principal::system(), svc);
    let cbu = insert_cbu(&mut scope, "cas-a1-manco").await;
    let manco = insert_entity(&mut scope, type_id, "cas-manco").await;
    let fund = insert_entity(&mut scope, type_id, "cas-fund").await;

    let args = |pct: &str, ev: Option<i64>| {
        let mut a = json!({ "cbu-id": cbu, "entity-id": manco, "fund-entity-id": fund,
                            "role": "MANAGEMENT_COMPANY", "investment-percentage": pct });
        if let Some(v) = ev {
            a["expected-version"] = json!(v);
        }
        a
    };

    AssignFundRole
        .execute(&args("10.00", None), &mut ctx, &mut scope)
        .await
        .expect("manco insert");
    assert_eq!(role_version(&mut scope, cbu).await, 1);
    // A deliberately WRONG version (999) would reject a Class-1 row — but the ManCo
    // row is Class 2 (carve-out), so it must update regardless of version.
    AssignFundRole
        .execute(&args("20.00", Some(999)), &mut ctx, &mut scope)
        .await
        .expect("ManCo (Class 2) write must NOT be version-blocked");
    assert_eq!(
        role_pct(&mut scope, cbu).await,
        Some("20.00".parse().unwrap()),
        "carve-out write applied"
    );
    assert_eq!(
        role_version(&mut scope, cbu).await,
        2,
        "carve-out still bumps version"
    );
    eprintln!("[CAS a1] ManCo carve-out proven: stale-version write applied (Class-2 not blocked)");
    drop(scope);
}

/// Class 1 vs Class 3 (A7) — an operator edge (`source` cbu.*) is CAS'd and a
/// stale write is rejected; a `ubo_graph` recompute edge (`source LIKE 'ubo.%'`)
/// is carved out and a stale write is NOT blocked.
#[tokio::test]
#[ignore = "Phase 1 §10.6 CAS: requires DATABASE_URL + database feature"]
async fn cas_a7_rejects_class1_and_carves_out_recompute() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = PgPool::connect(&url).await.expect("connect");
    let type_id = any_entity_type(&pool).await;
    let svc = services();
    let mut scope = PgTransactionScope::begin(&pool).await.expect("begin");
    let mut ctx = VerbExecutionContext::with_services(Principal::system(), svc);
    let a = insert_entity(&mut scope, type_id, "cas-a7-from").await;
    let b = insert_entity(&mut scope, type_id, "cas-a7-to").await;
    let c = insert_entity(&mut scope, type_id, "cas-a7-to2").await;

    // --- Class 1 (operator edge, no effective-from → NULL branch) ---
    let op_edge = |pct: &str, ev: Option<i64>| {
        let mut x = json!({ "from-entity-id": a, "to-entity-id": b, "relationship-type": "ownership",
                            "percentage": pct, "source": "cbu.assign-ownership" });
        if let Some(v) = ev {
            x["expected-version"] = json!(v);
        }
        x
    };
    EdgeUpsert
        .execute(&op_edge("30.00", None), &mut ctx, &mut scope)
        .await
        .expect("op edge insert");
    assert_eq!(edge_version(&mut scope, a, b, "ownership").await, 1);
    EdgeUpsert
        .execute(&op_edge("31.00", Some(1)), &mut ctx, &mut scope)
        .await
        .expect("matched update");
    assert_eq!(edge_version(&mut scope, a, b, "ownership").await, 2);
    let stale = EdgeUpsert
        .execute(&op_edge("99.00", Some(1)), &mut ctx, &mut scope)
        .await;
    assert!(
        stale.is_err(),
        "stale Class-1 edge write MUST be rejected, got {stale:?}"
    );
    assert!(format!("{}", stale.unwrap_err()).contains("CAS conflict"));
    eprintln!("[CAS a7] class-1 operator edge stale write rejected");

    // --- Class 3 (ubo recompute edge, WITH effective-from → effective_from branch) ---
    let ubo_edge = |pct: &str, ev: Option<i64>| {
        let mut x = json!({ "from-entity-id": a, "to-entity-id": c, "relationship-type": "ownership",
                            "percentage": pct, "source": "ubo.supersede", "effective-from": "2024-01-01" });
        if let Some(v) = ev {
            x["expected-version"] = json!(v);
        }
        x
    };
    EdgeUpsert
        .execute(&ubo_edge("40.00", None), &mut ctx, &mut scope)
        .await
        .expect("ubo edge insert");
    // Deliberately wrong version — recompute (Class 3) is carved out, must apply.
    EdgeUpsert
        .execute(&ubo_edge("41.00", Some(999)), &mut ctx, &mut scope)
        .await
        .expect("ubo recompute (Class 3) write must NOT be version-blocked");
    eprintln!("[CAS a7] ubo recompute carve-out proven: stale-version write applied (Class-3 not blocked)");
    drop(scope);
}
