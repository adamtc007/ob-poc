//! T7.1 gate tests — EOP-PLAN-KYCUBO-KIT-T7 §2, EOP-DD-KYCUBO-CAPTURE-CHARTER v0.1.
//!
//! `capture_records_accepted_proposal_roundtrip` proves persistence +
//! retrieval through the public recorder API. `capture_respects_charter_scope`
//! proves the table carries EXACTLY the charter §1 field list (`id` /
//! `created_at` excepted as bookkeeping) — not one field more, so a future
//! silent `ALTER TABLE` trips this test before it ever reaches review (I-6:
//! everything captured is governed).

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use ob_poc::domain_ops::kyc_ramp_capture::{record_capture, CaptureRecord, Disposition};
use semantic_decision_contracts::MoveAttemptOutcome;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn connect() -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("connect to test DB")
}

#[tokio::test]
async fn capture_records_accepted_proposal_roundtrip() {
    let pool = connect().await;
    let mut conn = pool.acquire().await.unwrap();

    let staged_move_id = Uuid::new_v4();
    let record = CaptureRecord {
        utterance_text: "register the new subject as a natural person".to_string(),
        placement_set_hash: "test-board-hash-abc123".to_string(),
        proposal: "(kyc.subject.register :subject <uuid>)".to_string(),
        disposition: Disposition::Select,
        user_action: MoveAttemptOutcome::Applied,
        staged_move_id: Some(staged_move_id),
    };

    let id = record_capture(&mut conn, &record)
        .await
        .expect("record_capture must succeed");

    let row = sqlx::query(
        r#"SELECT utterance_text, placement_set_hash, proposal, disposition, user_action, staged_move_id
           FROM "ob-poc".kyc_ramp_capture WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&mut *conn)
    .await
    .expect("row must be readable back");

    let utterance_text: String = row.get("utterance_text");
    let placement_set_hash: String = row.get("placement_set_hash");
    let proposal: String = row.get("proposal");
    let disposition: String = row.get("disposition");
    let user_action: String = row.get("user_action");
    let persisted_staged_move_id: Option<Uuid> = row.get("staged_move_id");

    assert_eq!(utterance_text, record.utterance_text);
    assert_eq!(placement_set_hash, record.placement_set_hash);
    assert_eq!(proposal, record.proposal);
    assert_eq!(disposition, "select");
    assert_eq!(user_action, "applied");
    assert_eq!(persisted_staged_move_id, Some(staged_move_id));

    sqlx::query(r#"DELETE FROM "ob-poc".kyc_ramp_capture WHERE id = $1"#)
        .bind(id)
        .execute(&mut *conn)
        .await
        .ok();
}

/// I-6 / charter §1: the table carries EXACTLY the charter's field list.
/// `id` and `created_at` are bookkeeping, not charter content. Any other
/// addition needs a charter amendment before it needs a migration.
#[tokio::test]
async fn capture_respects_charter_scope() {
    let pool = connect().await;

    const CHARTERED_COLUMNS: &[&str] = &[
        "id",
        "utterance_text",
        "placement_set_hash",
        "proposal",
        "disposition",
        "user_action",
        "staged_move_id",
        "created_at",
    ];

    let rows = sqlx::query(
        r#"SELECT column_name FROM information_schema.columns
           WHERE table_schema = 'ob-poc' AND table_name = 'kyc_ramp_capture'"#,
    )
    .fetch_all(&pool)
    .await
    .expect("must be able to introspect kyc_ramp_capture columns");

    let mut actual: Vec<String> = rows
        .iter()
        .map(|r| r.get::<String, _>("column_name"))
        .collect();
    actual.sort();
    let mut expected: Vec<String> = CHARTERED_COLUMNS.iter().map(|s| s.to_string()).collect();
    expected.sort();

    assert_eq!(
        actual, expected,
        "kyc_ramp_capture columns drifted from the charter's field list — extend \
         EOP-DD-KYCUBO-CAPTURE-CHARTER_v0.1.md §1 first, then this test, then the migration"
    );
}
