//! Phase 1 (§8) — `cbu.assign-role` execution-parity matrix.
//!
//! The I1 fold (2026-06-18) folded six `cbu.assign-*` specialist verbs into one
//! discoverable `cbu.assign-role :role-type=X`, dispatching to the RETAINED-but-
//! unregistered specialist op structs. v0.5 §8 / M-6: a shared destructive/
//! mutating definition that folds per-entity-canonical write-paths **needs
//! execution-parity tests** before it goes live. This is that owed gate.
//!
//! For each role-type, this asserts the dispatched path (`AssignRole` with
//! `role-type=X`) writes the compliance-bearing `cbu_entity_roles` columns
//! identically to calling the specialist op directly — by running both on two
//! CBUs that share the same entities/role/percentage and comparing the two rows
//! column-for-column (cbu_id, generated id, and timestamps excluded by design).
//! For the edge-bearing role-types it also asserts both paths resolve to the
//! same `entity_relationships` edge (`relationship_id`).
//!
//! PRIVILEGE CONTEXT (stated for the receipt, per the governance):
//!   * Connection: `DATABASE_URL` → role `adamtc007` (a PostgreSQL SUPERUSER).
//!   * RLS: `cbus`, `cbu_entity_roles`, `entity_relationships` have row-level
//!     security DISABLED (`relrowsecurity = f`, verified) — there is no RLS
//!     policy for a superuser to bypass on these tables.
//!   * Parity is PRIVILEGE-INVARIANT: both the dispatcher and the specialist
//!     execute under the IDENTICAL connection inside ONE transaction, so no
//!     privilege/bypass path can make a divergent write compare equal. The
//!     superuser connection cannot produce a false-green for a parity comparison.
//!   * All fixtures are created inside a `PgTransactionScope` that is dropped
//!     without commit → rolled back; the test leaves no rows behind.
//!
//! Run:
//!   DATABASE_URL="postgresql:///data_designer" \
//!     cargo test --features database --test cbu_assign_role_parity -- --ignored --nocapture

#![cfg(feature = "database")]

use std::sync::Arc;

use serde_json::{json, Value};
use sqlx::types::BigDecimal;
use sqlx::PgPool;
use uuid::Uuid;

use dsl_runtime::{
    SemOsChildDispatcher, ServiceRegistryBuilder, TransactionScope, VerbExecutionContext,
    VerbExecutionOutcome,
};
use sem_os_core::principal::Principal;
use ob_poc::sequencer_tx::PgTransactionScope;
use sem_os_postgres::ops::cbu_role::{
    AssignControl, AssignFundRole, AssignOwnership, AssignRole, AssignServiceProvider,
    AssignSignatory, AssignTrustRole,
};
use sem_os_postgres::ops::{build_registry, RegistryChildDispatcher, SemOsVerbOp};

/// The compliance-bearing comparable subset of a `cbu_entity_roles` row
/// (cbu_id / generated id / timestamps deliberately excluded).
#[derive(Debug, PartialEq, sqlx::FromRow)]
struct RoleRow {
    entity_id: Uuid,
    role_id: Uuid,
    target_entity_id: Option<Uuid>,
    ownership_percentage: Option<BigDecimal>,
    effective_from: Option<chrono::NaiveDate>,
    authority_limit: Option<BigDecimal>,
    authority_currency: Option<String>,
    requires_co_signatory: Option<bool>,
}

async fn read_role_row(scope: &mut dyn TransactionScope, cbu_id: Uuid) -> RoleRow {
    sqlx::query_as::<_, RoleRow>(
        r#"SELECT entity_id, role_id, target_entity_id, ownership_percentage,
                  effective_from, authority_limit, authority_currency, requires_co_signatory
           FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#,
    )
    .bind(cbu_id)
    .fetch_one(scope.executor())
    .await
    .expect("role row written")
}

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

fn rel_id(outcome: &VerbExecutionOutcome) -> Option<String> {
    let VerbExecutionOutcome::Record(v) = outcome else {
        return None;
    };
    v.get("relationship_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// One role-type parity case: (label, dispatcher role-type, the specialist op,
/// the role name, whether an entity_relationships edge is expected, and the
/// per-CBU arg builder).
struct Case {
    label: &'static str,
    role_type: Option<&'static str>,
    specialist: Arc<dyn SemOsVerbOp>,
    role: &'static str,
    expects_edge: bool,
    /// Build the args for one CBU given (cbu_id, primary_entity, target_entity).
    build_args: fn(Uuid, Uuid, Uuid) -> Value,
}

#[tokio::test]
#[ignore = "Phase 1 §8 parity: requires DATABASE_URL + database feature"]
async fn assign_role_dispatch_parity_matrix() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = PgPool::connect(&url).await.expect("connect");

    // State the live privilege context in the test output too (not only the docs).
    let (user, is_super): (String, bool) = sqlx::query_as(
        "select current_user, coalesce((select rolsuper from pg_roles where rolname = current_user), false)",
    )
    .fetch_one(&pool)
    .await
    .expect("priv check");
    eprintln!("[privilege] connected as '{user}', superuser={is_super}; target tables have RLS disabled; parity is privilege-invariant.");

    let type_id = any_entity_type(&pool).await;

    // Full registry → child dispatcher service (entity-relationship.upsert lives here).
    let mut registry = build_registry();
    ob_poc::domain_ops::extend_registry(&mut registry);
    let registry = Arc::new(registry);
    let dispatcher = Arc::new(RegistryChildDispatcher::new(registry.clone()));
    let mut sb = ServiceRegistryBuilder::new();
    sb.register::<dyn SemOsChildDispatcher>(dispatcher);
    let services = Arc::new(sb.build());

    let cases: Vec<Case> = vec![
        Case {
            label: "OWNERSHIP",
            role_type: Some("OWNERSHIP"),
            specialist: Arc::new(AssignOwnership),
            role: "SHAREHOLDER",
            expects_edge: true,
            build_args: |cbu, owner, owned| {
                json!({ "cbu-id": cbu, "owner-entity-id": owner, "owned-entity-id": owned,
                        "percentage": "42.50", "role": "SHAREHOLDER", "effective-from": "2024-01-01" })
            },
        },
        Case {
            label: "CONTROL",
            role_type: Some("CONTROL"),
            specialist: Arc::new(AssignControl),
            role: "DIRECTOR",
            expects_edge: true,
            build_args: |cbu, controller, controlled| {
                json!({ "cbu-id": cbu, "controller-entity-id": controller,
                        "controlled-entity-id": controlled, "role": "DIRECTOR",
                        "control-type": "BOARD", "appointment-date": "2024-02-02" })
            },
        },
        Case {
            label: "TRUST",
            role_type: Some("TRUST"),
            specialist: Arc::new(AssignTrustRole),
            role: "TRUSTEE",
            expects_edge: true,
            build_args: |cbu, participant, trust| {
                json!({ "cbu-id": cbu, "participant-entity-id": participant,
                        "trust-entity-id": trust, "role": "TRUSTEE",
                        "interest-percentage": "12.50", "interest-type": "FIXED" })
            },
        },
        Case {
            // NOTE: only fund roles mapping to relationship_type ∈
            // {ownership,control,trust_role,employment,management} pass
            // chk_er_relationship_type. MANAGEMENT_COMPANY → "management" (valid).
            // FUND_INVESTOR/FEEDER_FUND/SUB_FUND/PARALLEL_FUND map to types the
            // check constraint REJECTS — a pre-existing AssignFundRole issue the
            // fold preserves identically (parity holds; both paths fail the same).
            label: "FUND",
            role_type: Some("FUND"),
            specialist: Arc::new(AssignFundRole),
            role: "MANAGEMENT_COMPANY",
            expects_edge: true,
            build_args: |cbu, entity, fund| {
                json!({ "cbu-id": cbu, "entity-id": entity, "fund-entity-id": fund,
                        "role": "MANAGEMENT_COMPANY", "investment-percentage": "7.25" })
            },
        },
        Case {
            label: "SERVICE_PROVIDER",
            role_type: Some("SERVICE_PROVIDER"),
            specialist: Arc::new(AssignServiceProvider),
            role: "ADMINISTRATOR",
            expects_edge: false,
            build_args: |cbu, provider, client| {
                json!({ "cbu-id": cbu, "provider-entity-id": provider,
                        "client-entity-id": client, "role": "ADMINISTRATOR",
                        "service-agreement-date": "2024-03-03" })
            },
        },
        Case {
            label: "SIGNATORY",
            role_type: Some("SIGNATORY"),
            specialist: Arc::new(AssignSignatory),
            role: "AUTHORIZED_SIGNATORY",
            expects_edge: false,
            build_args: |cbu, person, for_entity| {
                json!({ "cbu-id": cbu, "person-entity-id": person, "for-entity-id": for_entity,
                        "role": "AUTHORIZED_SIGNATORY", "authority-limit": "1000000",
                        "authority-currency": "EUR", "requires-co-signatory": true })
            },
        },
        Case {
            // Generic default branch (role-type absent) — replicates the former
            // crud `cbu.assign-role` role_link insert.
            label: "GENERIC",
            role_type: None,
            specialist: Arc::new(AssignRole),
            role: "DIRECTOR",
            expects_edge: false,
            build_args: |cbu, entity, _unused| {
                json!({ "cbu-id": cbu, "entity-id": entity, "role": "DIRECTOR" })
            },
        },
    ];

    let mut checked = 0usize;
    for case in &cases {
        // Fresh transaction per case → fixtures roll back on drop.
        let mut scope = PgTransactionScope::begin(&pool).await.expect("begin scope");
        let mut ctx = VerbExecutionContext::with_services(Principal::system(), services.clone());

        let cbu_a = insert_cbu(&mut scope, &format!("parity-{}-A", case.label)).await;
        let cbu_b = insert_cbu(&mut scope, &format!("parity-{}-B", case.label)).await;
        // Shared entities so the two role rows differ ONLY by cbu_id.
        let primary = insert_entity(&mut scope, type_id, &format!("p-{}-primary", case.label)).await;
        let target = insert_entity(&mut scope, type_id, &format!("p-{}-target", case.label)).await;

        // Dispatcher path on CBU-A.
        let mut args_a = (case.build_args)(cbu_a, primary, target);
        if let Some(rt) = case.role_type {
            args_a["role-type"] = json!(rt);
        }
        let out_a = AssignRole
            .execute(&args_a, &mut ctx, &mut scope)
            .await
            .unwrap_or_else(|e| panic!("[{}] dispatcher path failed: {e}", case.label));

        // Specialist path on CBU-B (identical role args, no role-type key).
        let args_b = (case.build_args)(cbu_b, primary, target);
        let out_b = case
            .specialist
            .execute(&args_b, &mut ctx, &mut scope)
            .await
            .unwrap_or_else(|e| panic!("[{}] specialist path failed: {e}", case.label));

        let row_a = read_role_row(&mut scope, cbu_a).await;
        let row_b = read_role_row(&mut scope, cbu_b).await;

        assert_eq!(
            row_a, row_b,
            "[{}] dispatcher and specialist wrote DIFFERENT cbu_entity_roles columns",
            case.label
        );

        if case.expects_edge {
            let ra = rel_id(&out_a);
            let rb = rel_id(&out_b);
            assert!(
                ra.is_some() && rb.is_some(),
                "[{}] expected an entity_relationships edge from both paths (a={ra:?}, b={rb:?})",
                case.label
            );
            assert_eq!(
                ra, rb,
                "[{}] dispatcher and specialist resolved to DIFFERENT edges",
                case.label
            );
        }

        eprintln!(
            "[parity OK] {:<16} dispatcher==specialist on cbu_entity_roles{}",
            case.label,
            if case.expects_edge { " + same edge" } else { "" }
        );
        checked += 1;
        // scope dropped here → ROLLBACK (no commit). Nothing persisted.
        drop(scope);
    }

    assert_eq!(checked, 7, "all 7 role-type paths must be parity-checked");
    eprintln!("[parity] {checked}/7 role-type write-paths verified byte-identical (dispatcher vs specialist).");
}
