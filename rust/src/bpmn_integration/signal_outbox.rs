//! Best-effort "verb side-effect → correlated BPMN signal" notification.
//!
//! Extracted from `request_ops.rs`'s original `try_send_bpmn_signal`
//! (EOP-PLAN-DAG-AWAITS-001 Phase 5 addendum) — generalized so any domain
//! op can notify a waiting BPMN process instance, not just the KYC-case
//! lifecycle. Queues a `bpmn_signal` row into `public.outbox`; the existing
//! `BpmnSignalConsumer` drainer (registered alongside
//! `MaintenanceSpawnConsumer` in `ob-poc-web::main`) performs the actual
//! gRPC `Signal` call post-commit. Net effect: zero gRPC/HTTP inside the
//! verb body — if the outer txn rolls back, the outbox row is gone with it.

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Identifies the outbox row a successful [`queue_bpmn_signal`] call queued
/// — returned so callers that keep their own domain-specific audit trail
/// (e.g. `request_ops.rs`'s `communication_log`) can record it without this
/// generic helper needing to know about that trail's shape.
#[cfg(feature = "database")]
pub(crate) struct QueuedSignal {
    pub process_instance_id: uuid::Uuid,
    pub correlation_id: uuid::Uuid,
    pub outbox_id: uuid::Uuid,
    pub idempotency_key: String,
}

/// Queue a signal for the active BPMN process instance correlated by
/// `(process_key, correlation_key)`, if one exists. Returns `None` (logged,
/// not erroring) if there's no active correlation or the outbox insert
/// failed — callers use this for best-effort notification, never as the
/// sole path to a required effect.
#[cfg(feature = "database")]
pub(crate) async fn queue_bpmn_signal(
    pool: &PgPool,
    process_key: &str,
    correlation_key: &str,
    signal_name: &str,
    payload: &serde_json::Value,
) -> Option<QueuedSignal> {
    use super::correlation::CorrelationStore;

    let store = CorrelationStore::new(pool.clone());
    let correlation = match store
        .find_active_by_domain_key(process_key, correlation_key)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::debug!(
                process_key,
                correlation_key,
                signal = signal_name,
                "No active BPMN correlation, skipping signal"
            );
            return None;
        }
        Err(e) => {
            tracing::warn!(
                process_key,
                correlation_key,
                signal = signal_name,
                error = %e,
                "Failed to query BPMN correlation, skipping signal"
            );
            return None;
        }
    };

    // Idempotency key collapses duplicate signals from a single txn (e.g. a
    // retried verb execution).
    let outbox_id = uuid::Uuid::new_v4();
    let trace_id = uuid::Uuid::new_v4();
    let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();

    let mut hasher = blake3::Hasher::new();
    hasher.update(signal_name.as_bytes());
    hasher.update(b"\x00");
    hasher.update(&payload_bytes);
    let payload_hash = hasher.finalize().to_hex().to_string();
    let idempotency_key = format!(
        "bpmn_signal:{}:{}:{}",
        correlation.process_instance_id,
        signal_name,
        &payload_hash[..16]
    );

    let outbox_payload = serde_json::json!({
        "instance_id": correlation.process_instance_id,
        "message_name": signal_name,
        "payload": serde_json::to_string(payload).unwrap_or_default(),
        // The wait's own `zeebe:subscription correlationKey="=..."` value —
        // BpmnSignalConsumer needs this to call signal_with_correlation
        // rather than signal (whose `correlation_key: None` gets coerced
        // to the literal string "false" server-side and never matches a
        // real correlated wait).
        "correlation_key": correlation_key,
    });

    match sqlx::query(
        r#"
        INSERT INTO public.outbox
            (id, trace_id, envelope_version, effect_kind, payload, idempotency_key, status)
        VALUES
            ($1, $2, $3, $4, $5, $6, 'pending')
        ON CONFLICT (idempotency_key, effect_kind) DO NOTHING
        "#,
    )
    .bind(outbox_id)
    .bind(trace_id)
    .bind(1i16)
    .bind("bpmn_signal")
    .bind(&outbox_payload)
    .bind(&idempotency_key)
    .execute(pool)
    .await
    {
        Err(e) => {
            tracing::warn!(
                process_key,
                correlation_key,
                signal = signal_name,
                error = %e,
                "Failed to queue bpmn_signal in public.outbox (non-blocking)"
            );
            None
        }
        Ok(_) => {
            tracing::info!(
                process_key,
                correlation_key,
                process_instance_id = %correlation.process_instance_id,
                signal = signal_name,
                %idempotency_key,
                "bpmn.signal queued to public.outbox"
            );
            Some(QueuedSignal {
                process_instance_id: correlation.process_instance_id,
                correlation_id: correlation.correlation_id,
                outbox_id,
                idempotency_key,
            })
        }
    }
}
