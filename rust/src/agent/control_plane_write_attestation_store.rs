//! T5.3 (EOP-PLAN-CONTROLPLANE-001): persists write-set attestation records
//! to `"ob-poc".control_plane_write_attestations`. Called from
//! `PgTransactionScope::commit_attested` (`sequencer_tx.rs`) after the
//! commit/rollback decision is already made — this is audit trail, not
//! part of the enforcement mechanism itself.

use uuid::Uuid;

use ob_poc_control_plane::write_set_attestation::CapturedWrite;

/// Best-effort insert. Never returns `Err` — a persistence failure must
/// not affect the commit/rollback decision that already happened, matching
/// `agent::telemetry::store`'s and `agent::control_plane_shadow`'s posture.
#[cfg(feature = "database")]
pub(crate) async fn persist_attestation(
    pool: &sqlx::PgPool,
    scope_id: ob_poc_types::TransactionScopeId,
    session_id: Option<Uuid>,
    verb_fqn: Option<&str>,
    captured: &[CapturedWrite],
    bounded: bool,
    excess: &[CapturedWrite],
) -> bool {
    let captured_json = serde_json::to_value(captured).unwrap_or(serde_json::Value::Null);
    let excess_json = serde_json::to_value(excess).unwrap_or(serde_json::Value::Null);

    let result = sqlx::query(
        r#"
        INSERT INTO "ob-poc".control_plane_write_attestations (
            scope_id, session_id, verb_fqn, bounded, captured_writes, excess_writes
        ) VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(scope_id.0)
    .bind(session_id)
    .bind(verb_fqn)
    .bind(bounded)
    .bind(captured_json)
    .bind(excess_json)
    .execute(pool)
    .await;

    match result {
        Ok(_) => true,
        Err(err) => {
            tracing::warn!(
                error = %err,
                scope_id = %scope_id.0,
                bounded,
                "control_plane_write_attestations insert failed (best-effort, non-blocking)"
            );
            false
        }
    }
}
