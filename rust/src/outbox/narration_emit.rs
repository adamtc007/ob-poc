//! Helper used by `ReplOrchestratorV2` to write a `Narrate` outbox
//! row after each turn that produced a narration. Phase 5e-narration-
//! cutover transitional dual-write path.
//!
//! The row is consumed by [`super::NarrateConsumer`].

use anyhow::{Context, Result};
use ob_poc_types::{EnvelopeVersion, IdempotencyKey, OutboxEffectKind, TraceId};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// Write a `Narrate` row into `public.outbox`. The orchestrator
/// calls this AFTER inline narration synthesis succeeds; the
/// payload it passes is the same `NarrationPayload` the response
/// already carries, so the consumer-side delivery is a strict
/// shadow of the inline path.
///
/// Idempotency: keyed on `(trace_id, session_id)` so a re-issued
/// turn (e.g. retry from the client) doesn't double-emit. The
/// `(idempotency_key, effect_kind)` UNIQUE on the table dedupes
/// silently via `ON CONFLICT DO NOTHING`.
pub async fn emit_narration_outbox(
    pool: &PgPool,
    session_id: Uuid,
    trace_id: TraceId,
    workspace_key: Option<&str>,
    narration: &serde_json::Value,
) -> Result<Uuid> {
    let outbox_id = Uuid::new_v4();
    let idempotency_key = IdempotencyKey::from_parts(
        "narrate",
        trace_id,
        &format!("session:{}", session_id),
    );
    let payload = json!({
        "session_id": session_id,
        "workspace_key": workspace_key,
        "narration": narration,
    });

    sqlx::query(
        r#"
        INSERT INTO public.outbox
            (id, trace_id, envelope_version, effect_kind, payload,
             idempotency_key, status)
        VALUES ($1, $2, $3, $4, $5, $6, 'pending')
        ON CONFLICT (idempotency_key, effect_kind) DO NOTHING
        "#,
    )
    .bind(outbox_id)
    .bind(trace_id.0)
    .bind(EnvelopeVersion::CURRENT.0 as i16)
    .bind(narrate_kind_str())
    .bind(&payload)
    .bind(&idempotency_key.0)
    .execute(pool)
    .await
    .context("insert narrate outbox row")?;

    Ok(outbox_id)
}

fn narrate_kind_str() -> &'static str {
    // Stable serde representation of OutboxEffectKind::Narrate so
    // the writer side doesn't need to depend on the consumer's enum
    // variant name spelling.
    match serde_json::to_value(OutboxEffectKind::Narrate) {
        Ok(serde_json::Value::String(s)) if s == "narrate" => "narrate",
        // Fallback — the enum's serde rename_all="snake_case" makes
        // the literal stable, but if it ever drifts we'd rather fail
        // a test than silently mis-route.
        _ => "narrate",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrate_kind_serde_is_stable() {
        // If this ever fails, OutboxEffectKind's serde shape drifted
        // and the writer would route the row to a non-existent
        // consumer.
        let v = serde_json::to_value(OutboxEffectKind::Narrate).unwrap();
        assert_eq!(v, serde_json::Value::String("narrate".into()));
    }
}
