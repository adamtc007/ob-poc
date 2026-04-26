//! Tranche 3 Phase 3.E — Catalogue workspace lifecycle integration test.
//!
//! Live-DB end-to-end: propose → stage → commit → list. Exercises the
//! 4 authorship verbs against the v1.2 §6.2 strict catalogue + the
//! `catalogue_proposals` carrier table from migration
//! `20260427_catalogue_workspace.sql`.
//!
//! Two-eye rule paths exercised:
//! - Same-principal commit attempt → fails with two-eye violation.
//! - Different-principal commit → succeeds, projection write lands.
//!
//! Run: `DATABASE_URL=postgresql:///data_designer cargo test \
//!       --features database --test catalogue_workspace_lifecycle -- --ignored --nocapture`
//!
//! Marked `#[ignore]` to keep it out of the default `cargo test` run
//! since it requires a live Postgres + the catalogue migration applied.

#![cfg(feature = "database")]

use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use uuid::Uuid;

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for catalogue lifecycle integration test");
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("Postgres connect")
}

async fn cleanup(pool: &PgPool, verb_fqn: &str) {
    // Idempotent cleanup so the test is re-runnable.
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".catalogue_committed_verbs WHERE verb_fqn = $1"#)
        .bind(verb_fqn)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".catalogue_proposals WHERE verb_fqn = $1"#)
        .bind(verb_fqn)
        .execute(pool)
        .await;
}

#[tokio::test]
#[ignore]
async fn lifecycle_propose_stage_commit_succeeds() {
    let pool = pool().await;
    let verb_fqn = format!("test.lifecycle-{}", Uuid::new_v4());
    cleanup(&pool, &verb_fqn).await;

    // 1. Propose (DRAFT entry).
    let proposed_by = "alice@example.com";
    let declaration = json!({
        "description": "test verb for lifecycle smoke",
        "behavior": "plugin",
        "three_axis": {
            "state_effect": "preserving",
            "external_effects": ["observational"],
            "consequence": { "baseline": "benign" }
        }
    });
    let proposal_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".catalogue_proposals
           (verb_fqn, proposed_declaration, rationale, status, proposed_by)
           VALUES ($1, $2, $3, 'DRAFT', $4)
           RETURNING proposal_id"#,
    )
    .bind(&verb_fqn)
    .bind(&declaration)
    .bind("test propose")
    .bind(proposed_by)
    .fetch_one(&pool)
    .await
    .expect("propose insert");

    // 2. Auto-stage (validator-clean checkpoint).
    sqlx::query(
        r#"UPDATE "ob-poc".catalogue_proposals SET status='STAGED', staged_at=now() WHERE proposal_id=$1"#,
    )
    .bind(proposal_id)
    .execute(&pool)
    .await
    .expect("stage");

    // 3. Commit by a DIFFERENT principal (two-eye rule).
    let approver = "bob@example.com";
    let mut tx = pool.begin().await.expect("begin tx");
    sqlx::query(
        r#"UPDATE "ob-poc".catalogue_proposals
           SET status='COMMITTED', committed_by=$2, committed_at=now()
           WHERE proposal_id=$1"#,
    )
    .bind(proposal_id)
    .bind(approver)
    .execute(&mut *tx)
    .await
    .expect("commit update");
    sqlx::query(
        r#"INSERT INTO "ob-poc".catalogue_committed_verbs
           (verb_fqn, declaration, committed_proposal_id)
           VALUES ($1, $2, $3)
           ON CONFLICT (verb_fqn) DO UPDATE
             SET declaration = EXCLUDED.declaration,
                 committed_proposal_id = EXCLUDED.committed_proposal_id,
                 committed_at = now()"#,
    )
    .bind(&verb_fqn)
    .bind(&declaration)
    .bind(proposal_id)
    .execute(&mut *tx)
    .await
    .expect("commit projection");
    tx.commit().await.expect("commit tx");

    // 4. Verify final state.
    let row = sqlx::query(
        r#"SELECT status, committed_by FROM "ob-poc".catalogue_proposals WHERE proposal_id=$1"#,
    )
    .bind(proposal_id)
    .fetch_one(&pool)
    .await
    .expect("fetch final");
    let final_status: String = row.try_get("status").unwrap();
    let final_committer: String = row.try_get("committed_by").unwrap();
    assert_eq!(final_status, "COMMITTED");
    assert_eq!(final_committer, approver);

    // 5. Verify projection.
    let projection: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".catalogue_committed_verbs WHERE verb_fqn=$1"#,
    )
    .bind(&verb_fqn)
    .fetch_one(&pool)
    .await
    .expect("projection count");
    assert_eq!(projection, 1, "committed_verbs projection must have 1 row");

    cleanup(&pool, &verb_fqn).await;
}

#[tokio::test]
#[ignore]
async fn lifecycle_two_eye_rule_violation_blocks_commit() {
    let pool = pool().await;
    let verb_fqn = format!("test.two-eye-{}", Uuid::new_v4());
    cleanup(&pool, &verb_fqn).await;

    let proposed_by = "alice@example.com";
    let declaration = json!({"description": "two-eye test", "behavior": "plugin"});
    let proposal_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".catalogue_proposals
           (verb_fqn, proposed_declaration, status, proposed_by)
           VALUES ($1, $2, 'STAGED', $3)
           RETURNING proposal_id"#,
    )
    .bind(&verb_fqn)
    .bind(&declaration)
    .bind(proposed_by)
    .fetch_one(&pool)
    .await
    .expect("propose+stage");

    // Attempt to commit with the SAME principal as proposer — DB CHECK
    // constraint must reject.
    let result = sqlx::query(
        r#"UPDATE "ob-poc".catalogue_proposals
           SET status='COMMITTED', committed_by=$2, committed_at=now()
           WHERE proposal_id=$1"#,
    )
    .bind(proposal_id)
    .bind(proposed_by) // same principal — violates two-eye rule
    .execute(&pool)
    .await;

    assert!(
        result.is_err(),
        "two-eye rule violation must reject the UPDATE (same principal proposer + committer)"
    );
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("catalogue_two_eye_rule") || err_msg.contains("CHECK"),
        "expected CHECK constraint violation, got: {}",
        err_msg
    );

    cleanup(&pool, &verb_fqn).await;
}

#[tokio::test]
#[ignore]
async fn lifecycle_rollback_returns_to_terminal() {
    let pool = pool().await;
    let verb_fqn = format!("test.rollback-{}", Uuid::new_v4());
    cleanup(&pool, &verb_fqn).await;

    let proposal_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".catalogue_proposals
           (verb_fqn, proposed_declaration, status, proposed_by)
           VALUES ($1, $2, 'STAGED', 'alice@example.com')
           RETURNING proposal_id"#,
    )
    .bind(&verb_fqn)
    .bind(json!({"description": "rollback test", "behavior": "plugin"}))
    .fetch_one(&pool)
    .await
    .expect("propose+stage");

    sqlx::query(
        r#"UPDATE "ob-poc".catalogue_proposals
           SET status='ROLLED_BACK', rolled_back_by='alice@example.com',
               rolled_back_at=now(), rolled_back_reason='changed mind'
           WHERE proposal_id=$1"#,
    )
    .bind(proposal_id)
    .execute(&pool)
    .await
    .expect("rollback");

    let status: String = sqlx::query_scalar(
        r#"SELECT status FROM "ob-poc".catalogue_proposals WHERE proposal_id=$1"#,
    )
    .bind(proposal_id)
    .fetch_one(&pool)
    .await
    .expect("fetch");
    assert_eq!(status, "ROLLED_BACK");

    cleanup(&pool, &verb_fqn).await;
}
